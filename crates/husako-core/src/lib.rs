pub mod plugin;
pub mod progress;
pub mod quantity;
pub mod schema_source;
pub mod validate;
pub mod version_check;

use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use husako_runtime_qjs::ExecuteOptions;

use progress::ProgressReporter;

#[derive(Debug, thiserror::Error)]
pub enum HusakoError {
    #[error(transparent)]
    Compile(#[from] husako_compile_oxc::CompileError),
    #[error(transparent)]
    Runtime(#[from] husako_runtime_qjs::RuntimeError),
    #[error(transparent)]
    Emit(#[from] husako_yaml::EmitError),
    #[error(transparent)]
    OpenApi(#[from] husako_openapi::OpenApiError),
    #[error(transparent)]
    Dts(#[from] husako_dts::DtsError),
    #[error(transparent)]
    Config(#[from] husako_config::ConfigError),
    #[error(transparent)]
    Chart(#[from] husako_helm::HelmError),
    #[error("{0}")]
    Validation(String),
    #[error("generate I/O error: {0}")]
    GenerateIo(String),
}

pub struct RenderOptions {
    pub project_root: PathBuf,
    pub allow_outside_root: bool,
    pub schema_store: Option<validate::SchemaStore>,
    pub timeout_ms: Option<u64>,
    pub max_heap_mb: Option<usize>,
    pub verbose: bool,
}

pub fn render(
    source: &str,
    filename: &str,
    options: &RenderOptions,
) -> Result<String, HusakoError> {
    let js = husako_compile_oxc::compile(source, filename)?;

    if options.verbose {
        eprintln!(
            "[compile] {} ({} bytes → {} bytes JS)",
            filename,
            source.len(),
            js.len()
        );
    }

    let entry_path = std::path::Path::new(filename)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(filename));

    let generated_types_dir = options
        .project_root
        .join(".husako/types")
        .canonicalize()
        .ok();

    let plugin_modules = load_plugin_modules(&options.project_root);

    let exec_options = ExecuteOptions {
        entry_path,
        project_root: options.project_root.clone(),
        allow_outside_root: options.allow_outside_root,
        timeout_ms: options.timeout_ms,
        max_heap_mb: options.max_heap_mb,
        generated_types_dir,
        plugin_modules,
    };

    if options.verbose {
        eprintln!(
            "[execute] QuickJS: timeout={}ms, heap={}MB",
            options
                .timeout_ms
                .map_or("none".to_string(), |ms| ms.to_string()),
            options
                .max_heap_mb
                .map_or("none".to_string(), |mb| mb.to_string()),
        );
    }

    let execute_start = std::time::Instant::now();
    let value = husako_runtime_qjs::execute(&js, &exec_options)?;

    if options.verbose {
        eprintln!("[execute] done ({}ms)", execute_start.elapsed().as_millis());
    }

    let validate_mode = if options.schema_store.is_some() {
        "schema-based"
    } else {
        "fallback"
    };
    let doc_count = if let serde_json::Value::Array(arr) = &value {
        arr.len()
    } else {
        1
    };

    if options.verbose {
        eprintln!("[validate] {} documents, {}", doc_count, validate_mode);
    }

    let validate_start = std::time::Instant::now();
    if let Err(errors) = validate::validate(&value, options.schema_store.as_ref()) {
        let msg = errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        return Err(HusakoError::Validation(msg));
    }

    if options.verbose {
        eprintln!(
            "[validate] done ({}ms), 0 errors",
            validate_start.elapsed().as_millis()
        );
    }

    let yaml = husako_yaml::emit_yaml(&value)?;

    if options.verbose {
        let line_count = yaml.lines().count();
        eprintln!("[emit] {} documents ({} lines YAML)", doc_count, line_count);
    }

    Ok(yaml)
}

/// Load a `SchemaStore` from `.husako/types/k8s/_schema.json` if it exists.
pub fn load_schema_store(project_root: &Path) -> Option<validate::SchemaStore> {
    validate::load_schema_store(project_root)
}

pub struct GenerateOptions {
    pub project_root: PathBuf,
    /// CLI override for OpenAPI source (legacy mode).
    pub openapi: Option<husako_openapi::FetchOptions>,
    pub skip_k8s: bool,
    /// Config from `husako.toml` (config-driven mode).
    pub config: Option<husako_config::HusakoConfig>,
}

pub fn generate(
    options: &GenerateOptions,
    progress: &dyn ProgressReporter,
) -> Result<(), HusakoError> {
    let types_dir = options.project_root.join(".husako/types");

    // 1. Process plugins: install and collect presets
    let installed_plugins = if let Some(config) = &options.config
        && !config.plugins.is_empty()
    {
        plugin::install_plugins(config, &options.project_root, progress)?
    } else {
        Vec::new()
    };

    // Clone config and merge plugin presets (resources + charts)
    let mut merged_config = options.config.clone();
    if !installed_plugins.is_empty()
        && let Some(ref mut cfg) = merged_config
    {
        plugin::merge_plugin_presets(cfg, &installed_plugins);
    }

    // 2. Write static husako.d.ts
    write_file(&types_dir.join("husako.d.ts"), husako_sdk::HUSAKO_DTS)?;

    // 3. Write static husako/_base.d.ts
    write_file(
        &types_dir.join("husako/_base.d.ts"),
        husako_sdk::HUSAKO_BASE_DTS,
    )?;

    // 4. Generate k8s types
    // Priority: --skip-k8s → CLI flags → husako.toml [schemas] → skip
    if !options.skip_k8s {
        let specs = if let Some(openapi_opts) = &options.openapi {
            // Legacy CLI mode
            let task = progress.start_task("Fetching OpenAPI specs...");
            let client = husako_openapi::OpenApiClient::new(husako_openapi::FetchOptions {
                source: match &openapi_opts.source {
                    husako_openapi::OpenApiSource::Url {
                        base_url,
                        bearer_token,
                    } => husako_openapi::OpenApiSource::Url {
                        base_url: base_url.clone(),
                        bearer_token: bearer_token.clone(),
                    },
                    husako_openapi::OpenApiSource::Directory(p) => {
                        husako_openapi::OpenApiSource::Directory(p.clone())
                    }
                },
                cache_dir: options.project_root.join(".husako/cache"),
                offline: openapi_opts.offline,
            })?;
            let result = client.fetch_all_specs()?;
            task.finish_ok("Fetched OpenAPI specs");
            Some(result)
        } else if let Some(config) = &merged_config
            && !config.resources.is_empty()
        {
            // Config-driven mode (includes merged plugin presets)
            let cache_dir = options.project_root.join(".husako/cache");
            Some(schema_source::resolve_all(
                config,
                &options.project_root,
                &cache_dir,
                progress,
            )?)
        } else {
            None
        };

        if let Some(specs) = specs {
            let task = progress.start_task("Generating types...");
            let gen_options = husako_dts::GenerateOptions { specs };
            let result = husako_dts::generate(&gen_options)?;

            for (rel_path, content) in &result.files {
                write_file(&types_dir.join(rel_path), content)?;
            }
            task.finish_ok("Generated k8s types");
        }
    }

    // 5. Generate chart (helm) types from [charts] config (includes merged plugin charts)
    if let Some(config) = &merged_config
        && !config.charts.is_empty()
    {
        let cache_dir = options.project_root.join(".husako/cache");
        let chart_schemas =
            husako_helm::resolve_all(&config.charts, &options.project_root, &cache_dir)?;

        for (chart_name, schema) in &chart_schemas {
            let task = progress.start_task(&format!("Generating {chart_name} chart types..."));
            let (dts, js) = husako_dts::json_schema::generate_chart_types(chart_name, schema)?;
            write_file(&types_dir.join(format!("helm/{chart_name}.d.ts")), &dts)?;
            write_file(&types_dir.join(format!("helm/{chart_name}.js")), &js)?;
            task.finish_ok(&format!("{chart_name}: chart types generated"));
        }
    }

    // 6. Write/update tsconfig.json (includes plugin module paths)
    let plugin_paths = plugin::plugin_tsconfig_paths(&installed_plugins);
    write_tsconfig(&options.project_root, merged_config.as_ref(), &plugin_paths)?;

    Ok(())
}

fn write_file(path: &std::path::Path, content: &str) -> Result<(), HusakoError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            HusakoError::GenerateIo(format!("create dir {}: {e}", parent.display()))
        })?;
    }
    std::fs::write(path, content)
        .map_err(|e| HusakoError::GenerateIo(format!("write {}: {e}", path.display())))
}

fn write_tsconfig(
    project_root: &std::path::Path,
    config: Option<&husako_config::HusakoConfig>,
    plugin_paths: &std::collections::HashMap<String, String>,
) -> Result<(), HusakoError> {
    let tsconfig_path = project_root.join("tsconfig.json");

    let mut paths = serde_json::json!({
        "husako": [".husako/types/husako.d.ts"],
        "husako/_base": [".husako/types/husako/_base.d.ts"],
        "k8s/*": [".husako/types/k8s/*"]
    });

    // Add helm/* path if charts are configured
    if let Some(cfg) = config
        && !cfg.charts.is_empty()
    {
        paths.as_object_mut().unwrap().insert(
            "helm/*".to_string(),
            serde_json::json!([".husako/types/helm/*"]),
        );
    }

    // Add plugin module paths
    for (specifier, dts_path) in plugin_paths {
        paths
            .as_object_mut()
            .unwrap()
            .insert(specifier.clone(), serde_json::json!([dts_path]));
    }

    let husako_paths = paths;

    let config = if tsconfig_path.exists() {
        let content = std::fs::read_to_string(&tsconfig_path).map_err(|e| {
            HusakoError::GenerateIo(format!("read {}: {e}", tsconfig_path.display()))
        })?;

        let stripped = strip_jsonc(&content);
        match serde_json::from_str::<serde_json::Value>(&stripped) {
            Ok(mut root) => {
                // Merge paths into existing compilerOptions
                let compiler_options = root
                    .as_object_mut()
                    .and_then(|obj| {
                        if !obj.contains_key("compilerOptions") {
                            obj.insert("compilerOptions".to_string(), serde_json::json!({}));
                        }
                        obj.get_mut("compilerOptions")
                    })
                    .and_then(|co| co.as_object_mut());

                if let Some(co) = compiler_options {
                    co.entry("baseUrl")
                        .or_insert_with(|| serde_json::json!("."));

                    let paths = co.entry("paths").or_insert_with(|| serde_json::json!({}));
                    if let Some(paths_obj) = paths.as_object_mut()
                        && let Some(husako_obj) = husako_paths.as_object()
                    {
                        for (k, v) in husako_obj {
                            paths_obj.insert(k.clone(), v.clone());
                        }
                    }
                }

                root
            }
            Err(_) => {
                eprintln!("warning: could not parse existing tsconfig.json, creating new one");
                new_tsconfig(husako_paths)
            }
        }
    } else {
        new_tsconfig(husako_paths)
    };

    let formatted = serde_json::to_string_pretty(&config)
        .map_err(|e| HusakoError::GenerateIo(format!("serialize tsconfig.json: {e}")))?;

    std::fs::write(&tsconfig_path, formatted + "\n")
        .map_err(|e| HusakoError::GenerateIo(format!("write {}: {e}", tsconfig_path.display())))
}

// --- husako new ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateName {
    Simple,
    Project,
    MultiEnv,
}

impl FromStr for TemplateName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "simple" => Ok(Self::Simple),
            "project" => Ok(Self::Project),
            "multi-env" => Ok(Self::MultiEnv),
            _ => Err(format!(
                "unknown template '{s}'. Available: simple, project, multi-env"
            )),
        }
    }
}

impl fmt::Display for TemplateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Simple => write!(f, "simple"),
            Self::Project => write!(f, "project"),
            Self::MultiEnv => write!(f, "multi-env"),
        }
    }
}

pub struct ScaffoldOptions {
    pub directory: PathBuf,
    pub template: TemplateName,
    pub k8s_version: String,
}

pub fn scaffold(options: &ScaffoldOptions) -> Result<(), HusakoError> {
    let dir = &options.directory;

    // Reject non-empty existing directories
    if dir.exists() {
        let is_empty = std::fs::read_dir(dir)
            .map_err(|e| HusakoError::GenerateIo(format!("read dir {}: {e}", dir.display())))?
            .next()
            .is_none();
        if !is_empty {
            return Err(HusakoError::GenerateIo(format!(
                "directory '{}' is not empty",
                dir.display()
            )));
        }
    }

    // Create directory
    std::fs::create_dir_all(dir)
        .map_err(|e| HusakoError::GenerateIo(format!("create dir {}: {e}", dir.display())))?;

    // Write .gitignore (shared across all templates)
    write_file(&dir.join(".gitignore"), husako_sdk::TEMPLATE_GITIGNORE)?;

    let config_content = match options.template {
        TemplateName::Simple => husako_sdk::TEMPLATE_SIMPLE_CONFIG,
        TemplateName::Project => husako_sdk::TEMPLATE_PROJECT_CONFIG,
        TemplateName::MultiEnv => husako_sdk::TEMPLATE_MULTI_ENV_CONFIG,
    };
    let config_content = config_content.replace("%K8S_VERSION%", &options.k8s_version);
    write_file(&dir.join(husako_config::CONFIG_FILENAME), &config_content)?;

    match options.template {
        TemplateName::Simple => {
            write_file(&dir.join("entry.ts"), husako_sdk::TEMPLATE_SIMPLE_ENTRY)?;
        }
        TemplateName::Project => {
            write_file(
                &dir.join("env/dev.ts"),
                husako_sdk::TEMPLATE_PROJECT_ENV_DEV,
            )?;
            write_file(
                &dir.join("deployments/nginx.ts"),
                husako_sdk::TEMPLATE_PROJECT_DEPLOY_NGINX,
            )?;
            write_file(
                &dir.join("lib/index.ts"),
                husako_sdk::TEMPLATE_PROJECT_LIB_INDEX,
            )?;
            write_file(
                &dir.join("lib/metadata.ts"),
                husako_sdk::TEMPLATE_PROJECT_LIB_METADATA,
            )?;
        }
        TemplateName::MultiEnv => {
            write_file(
                &dir.join("base/nginx.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_BASE_NGINX,
            )?;
            write_file(
                &dir.join("base/service.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_BASE_SERVICE,
            )?;
            write_file(
                &dir.join("dev/main.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_DEV_MAIN,
            )?;
            write_file(
                &dir.join("staging/main.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_STAGING_MAIN,
            )?;
            write_file(
                &dir.join("release/main.ts"),
                husako_sdk::TEMPLATE_MULTI_ENV_RELEASE_MAIN,
            )?;
        }
    }

    Ok(())
}

// --- husako init ---

#[derive(Debug)]
pub struct InitOptions {
    pub directory: PathBuf,
    pub template: TemplateName,
    pub k8s_version: String,
}

pub fn init(options: &InitOptions) -> Result<(), HusakoError> {
    let dir = &options.directory;

    // Error if husako.toml already exists
    if dir.join(husako_config::CONFIG_FILENAME).exists() {
        return Err(HusakoError::GenerateIo(
            "husako.toml already exists. Use 'husako new <dir>' to create a new project."
                .to_string(),
        ));
    }

    // Write .gitignore: skip if exists, append .husako/ line if missing
    let gitignore_path = dir.join(".gitignore");
    if gitignore_path.exists() {
        let content = std::fs::read_to_string(&gitignore_path).unwrap_or_default();
        if !content.lines().any(|l| l.trim() == ".husako/") {
            let mut appended = content;
            if !appended.ends_with('\n') && !appended.is_empty() {
                appended.push('\n');
            }
            appended.push_str(".husako/\n");
            std::fs::write(&gitignore_path, appended).map_err(|e| {
                HusakoError::GenerateIo(format!("write {}: {e}", gitignore_path.display()))
            })?;
        }
    } else {
        write_file(&gitignore_path, husako_sdk::TEMPLATE_GITIGNORE)?;
    }

    let config_content = match options.template {
        TemplateName::Simple => husako_sdk::TEMPLATE_SIMPLE_CONFIG,
        TemplateName::Project => husako_sdk::TEMPLATE_PROJECT_CONFIG,
        TemplateName::MultiEnv => husako_sdk::TEMPLATE_MULTI_ENV_CONFIG,
    };
    let config_content = config_content.replace("%K8S_VERSION%", &options.k8s_version);
    write_file(&dir.join(husako_config::CONFIG_FILENAME), &config_content)?;

    match options.template {
        TemplateName::Simple => {
            let entry_path = dir.join("entry.ts");
            if !entry_path.exists() {
                write_file(&entry_path, husako_sdk::TEMPLATE_SIMPLE_ENTRY)?;
            }
        }
        TemplateName::Project => {
            let files = [
                ("env/dev.ts", husako_sdk::TEMPLATE_PROJECT_ENV_DEV),
                (
                    "deployments/nginx.ts",
                    husako_sdk::TEMPLATE_PROJECT_DEPLOY_NGINX,
                ),
                ("lib/index.ts", husako_sdk::TEMPLATE_PROJECT_LIB_INDEX),
                ("lib/metadata.ts", husako_sdk::TEMPLATE_PROJECT_LIB_METADATA),
            ];
            for (path, content) in files {
                let full_path = dir.join(path);
                if !full_path.exists() {
                    write_file(&full_path, content)?;
                }
            }
        }
        TemplateName::MultiEnv => {
            let files = [
                ("base/nginx.ts", husako_sdk::TEMPLATE_MULTI_ENV_BASE_NGINX),
                (
                    "base/service.ts",
                    husako_sdk::TEMPLATE_MULTI_ENV_BASE_SERVICE,
                ),
                ("dev/main.ts", husako_sdk::TEMPLATE_MULTI_ENV_DEV_MAIN),
                (
                    "staging/main.ts",
                    husako_sdk::TEMPLATE_MULTI_ENV_STAGING_MAIN,
                ),
                (
                    "release/main.ts",
                    husako_sdk::TEMPLATE_MULTI_ENV_RELEASE_MAIN,
                ),
            ];
            for (path, content) in files {
                let full_path = dir.join(path);
                if !full_path.exists() {
                    write_file(&full_path, content)?;
                }
            }
        }
    }

    Ok(())
}

// --- husako clean ---

#[derive(Debug)]
pub struct CleanOptions {
    pub project_root: PathBuf,
    pub cache: bool,
    pub types: bool,
}

#[derive(Debug)]
pub struct CleanResult {
    pub cache_removed: bool,
    pub types_removed: bool,
    pub cache_size: u64,
    pub types_size: u64,
}

pub fn clean(options: &CleanOptions) -> Result<CleanResult, HusakoError> {
    let husako_dir = options.project_root.join(".husako");
    let cache_dir = husako_dir.join("cache");
    let types_dir = husako_dir.join("types");

    let mut result = CleanResult {
        cache_removed: false,
        types_removed: false,
        cache_size: 0,
        types_size: 0,
    };

    if options.cache && options.types {
        // Full clean (--all): remove the entire .husako/ directory so nothing is left behind.
        if cache_dir.exists() {
            result.cache_size = dir_size(&cache_dir);
            result.cache_removed = true;
        }
        if types_dir.exists() {
            result.types_size = dir_size(&types_dir);
            result.types_removed = true;
        }
        if husako_dir.exists() {
            std::fs::remove_dir_all(&husako_dir).map_err(|e| {
                HusakoError::GenerateIo(format!("remove {}: {e}", husako_dir.display()))
            })?;
        }
    } else {
        if options.cache && cache_dir.exists() {
            result.cache_size = dir_size(&cache_dir);
            std::fs::remove_dir_all(&cache_dir).map_err(|e| {
                HusakoError::GenerateIo(format!("remove {}: {e}", cache_dir.display()))
            })?;
            result.cache_removed = true;
        }

        if options.types && types_dir.exists() {
            result.types_size = dir_size(&types_dir);
            std::fs::remove_dir_all(&types_dir).map_err(|e| {
                HusakoError::GenerateIo(format!("remove {}: {e}", types_dir.display()))
            })?;
            result.types_removed = true;
        }
    }

    Ok(result)
}

fn dir_size(path: &Path) -> u64 {
    walkdir(path).unwrap_or(0)
}

fn walkdir(path: &Path) -> Result<u64, std::io::Error> {
    let mut total = 0;
    if path.is_file() {
        return Ok(path.metadata()?.len());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let meta = entry.metadata()?;
        if meta.is_dir() {
            total += walkdir(&entry.path())?;
        } else {
            total += meta.len();
        }
    }
    Ok(total)
}

// --- husako list ---

#[derive(Debug)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub module_count: usize,
}

#[derive(Debug)]
pub struct DependencyList {
    pub resources: Vec<DependencyInfo>,
    pub charts: Vec<DependencyInfo>,
    pub plugins: Vec<PluginInfo>,
}

#[derive(Debug)]
pub struct DependencyInfo {
    pub name: String,
    pub source_type: &'static str,
    pub version: Option<String>,
    pub details: String,
}

pub fn list_dependencies(project_root: &Path) -> Result<DependencyList, HusakoError> {
    let config = husako_config::load(project_root)?;

    let mut resources = Vec::new();
    let mut charts = Vec::new();
    let mut plugins = Vec::new();

    if let Some(cfg) = &config {
        let mut res_entries: Vec<_> = cfg.resources.iter().collect();
        res_entries.sort_by_key(|(k, _)| k.as_str());
        for (name, source) in res_entries {
            resources.push(resource_info(name, source));
        }

        let mut chart_entries: Vec<_> = cfg.charts.iter().collect();
        chart_entries.sort_by_key(|(k, _)| k.as_str());
        for (name, source) in chart_entries {
            charts.push(chart_info(name, source));
        }
    }

    // List installed plugins
    for p in plugin::list_plugins(project_root) {
        plugins.push(PluginInfo {
            name: p.name,
            version: p.manifest.plugin.version,
            description: p.manifest.plugin.description,
            module_count: p.manifest.modules.len(),
        });
    }

    Ok(DependencyList {
        resources,
        charts,
        plugins,
    })
}

fn resource_info(name: &str, source: &husako_config::SchemaSource) -> DependencyInfo {
    match source {
        husako_config::SchemaSource::Release { version } => DependencyInfo {
            name: name.to_string(),
            source_type: "release",
            version: Some(version.clone()),
            details: String::new(),
        },
        husako_config::SchemaSource::Cluster { cluster } => DependencyInfo {
            name: name.to_string(),
            source_type: "cluster",
            version: None,
            details: cluster
                .as_deref()
                .map(|c| format!("cluster: {c}"))
                .unwrap_or_default(),
        },
        husako_config::SchemaSource::Git { repo, tag, path } => DependencyInfo {
            name: name.to_string(),
            source_type: "git",
            version: Some(tag.clone()),
            details: format!("{repo} ({})", path),
        },
        husako_config::SchemaSource::File { path } => DependencyInfo {
            name: name.to_string(),
            source_type: "file",
            version: None,
            details: path.clone(),
        },
    }
}

fn chart_info(name: &str, source: &husako_config::ChartSource) -> DependencyInfo {
    match source {
        husako_config::ChartSource::Registry {
            repo,
            chart,
            version,
        } => DependencyInfo {
            name: name.to_string(),
            source_type: "registry",
            version: Some(version.clone()),
            details: format!("{repo} ({})", chart),
        },
        husako_config::ChartSource::ArtifactHub { package, version } => DependencyInfo {
            name: name.to_string(),
            source_type: "artifacthub",
            version: Some(version.clone()),
            details: package.clone(),
        },
        husako_config::ChartSource::File { path } => DependencyInfo {
            name: name.to_string(),
            source_type: "file",
            version: None,
            details: path.clone(),
        },
        husako_config::ChartSource::Git { repo, tag, path } => DependencyInfo {
            name: name.to_string(),
            source_type: "git",
            version: Some(tag.clone()),
            details: format!("{repo} ({})", path),
        },
        husako_config::ChartSource::Oci { reference, version } => DependencyInfo {
            name: name.to_string(),
            source_type: "oci",
            version: Some(version.clone()),
            details: reference.clone(),
        },
    }
}

// --- husako add / remove (M17) ---

#[derive(Debug)]
pub enum AddTarget {
    Resource {
        name: String,
        source: husako_config::SchemaSource,
    },
    Chart {
        name: String,
        source: husako_config::ChartSource,
    },
}

pub fn add_dependency(project_root: &Path, target: &AddTarget) -> Result<(), HusakoError> {
    let (mut doc, path) = husako_config::edit::load_document(project_root)?;

    match target {
        AddTarget::Resource { name, source } => {
            husako_config::edit::add_resource(&mut doc, name, source);
        }
        AddTarget::Chart { name, source } => {
            husako_config::edit::add_chart(&mut doc, name, source);
        }
    }

    husako_config::edit::save_document(&doc, &path)?;
    Ok(())
}

#[derive(Debug)]
pub struct RemoveResult {
    pub name: String,
    pub section: &'static str,
}

pub fn remove_dependency(project_root: &Path, name: &str) -> Result<RemoveResult, HusakoError> {
    let (mut doc, path) = husako_config::edit::load_document(project_root)?;

    if husako_config::edit::remove_resource(&mut doc, name) {
        husako_config::edit::save_document(&doc, &path)?;
        return Ok(RemoveResult {
            name: name.to_string(),
            section: "resources",
        });
    }

    if husako_config::edit::remove_chart(&mut doc, name) {
        husako_config::edit::save_document(&doc, &path)?;
        return Ok(RemoveResult {
            name: name.to_string(),
            section: "charts",
        });
    }

    Err(HusakoError::Config(husako_config::ConfigError::Validation(
        format!("dependency '{name}' not found in [resources] or [charts]"),
    )))
}

// --- husako outdated (M18) ---

#[derive(Debug)]
pub struct OutdatedEntry {
    pub name: String,
    pub kind: &'static str,
    pub source_type: &'static str,
    pub current: String,
    pub latest: Option<String>,
    pub up_to_date: bool,
}

pub fn check_outdated(
    project_root: &Path,
    progress: &dyn ProgressReporter,
) -> Result<Vec<OutdatedEntry>, HusakoError> {
    let config = husako_config::load(project_root)?;
    let Some(cfg) = config else {
        return Ok(Vec::new());
    };

    let mut entries = Vec::new();

    for (name, source) in &cfg.resources {
        match source {
            husako_config::SchemaSource::Release { version } => {
                let task = progress.start_task(&format!("Checking {name}..."));
                match version_check::discover_latest_release() {
                    Ok(latest) => {
                        let up_to_date = version_check::versions_match(version, &latest);
                        task.finish_ok(&format!("{name}: {version} → {latest}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "resource",
                            source_type: "release",
                            current: version.clone(),
                            latest: Some(latest),
                            up_to_date,
                        });
                    }
                    Err(e) => {
                        task.finish_err(&format!("{name}: {e}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "resource",
                            source_type: "release",
                            current: version.clone(),
                            latest: None,
                            up_to_date: false,
                        });
                    }
                }
            }
            husako_config::SchemaSource::Git { tag, repo, .. } => {
                let task = progress.start_task(&format!("Checking {name}..."));
                match version_check::discover_latest_git_tag(repo) {
                    Ok(Some(latest)) => {
                        let up_to_date = tag == &latest;
                        task.finish_ok(&format!("{name}: {tag} → {latest}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "resource",
                            source_type: "git",
                            current: tag.clone(),
                            latest: Some(latest),
                            up_to_date,
                        });
                    }
                    Ok(None) => {
                        task.finish_ok(&format!("{name}: no tags"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "resource",
                            source_type: "git",
                            current: tag.clone(),
                            latest: None,
                            up_to_date: false,
                        });
                    }
                    Err(e) => {
                        task.finish_err(&format!("{name}: {e}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "resource",
                            source_type: "git",
                            current: tag.clone(),
                            latest: None,
                            up_to_date: false,
                        });
                    }
                }
            }
            // file/cluster have no version concept
            _ => {}
        }
    }

    for (name, source) in &cfg.charts {
        match source {
            husako_config::ChartSource::Registry {
                repo,
                chart,
                version,
            } => {
                let task = progress.start_task(&format!("Checking {name}..."));
                match version_check::discover_latest_registry(repo, chart) {
                    Ok(latest) => {
                        let up_to_date = version == &latest;
                        task.finish_ok(&format!("{name}: {version} → {latest}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "chart",
                            source_type: "registry",
                            current: version.clone(),
                            latest: Some(latest),
                            up_to_date,
                        });
                    }
                    Err(e) => {
                        task.finish_err(&format!("{name}: {e}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "chart",
                            source_type: "registry",
                            current: version.clone(),
                            latest: None,
                            up_to_date: false,
                        });
                    }
                }
            }
            husako_config::ChartSource::ArtifactHub { package, version } => {
                let task = progress.start_task(&format!("Checking {name}..."));
                match version_check::discover_latest_artifacthub(package) {
                    Ok(latest) => {
                        let up_to_date = version == &latest;
                        task.finish_ok(&format!("{name}: {version} → {latest}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "chart",
                            source_type: "artifacthub",
                            current: version.clone(),
                            latest: Some(latest),
                            up_to_date,
                        });
                    }
                    Err(e) => {
                        task.finish_err(&format!("{name}: {e}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "chart",
                            source_type: "artifacthub",
                            current: version.clone(),
                            latest: None,
                            up_to_date: false,
                        });
                    }
                }
            }
            husako_config::ChartSource::Git { tag, repo, .. } => {
                let task = progress.start_task(&format!("Checking {name}..."));
                match version_check::discover_latest_git_tag(repo) {
                    Ok(Some(latest)) => {
                        let up_to_date = tag == &latest;
                        task.finish_ok(&format!("{name}: {tag} → {latest}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "chart",
                            source_type: "git",
                            current: tag.clone(),
                            latest: Some(latest),
                            up_to_date,
                        });
                    }
                    Ok(None) => {
                        task.finish_ok(&format!("{name}: no tags"));
                    }
                    Err(e) => {
                        task.finish_err(&format!("{name}: {e}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "chart",
                            source_type: "git",
                            current: tag.clone(),
                            latest: None,
                            up_to_date: false,
                        });
                    }
                }
            }
            husako_config::ChartSource::Oci { reference, version } => {
                let task = progress.start_task(&format!("Checking {name}..."));
                match version_check::discover_latest_oci(reference) {
                    Ok(Some(latest)) => {
                        let up_to_date = version == &latest;
                        task.finish_ok(&format!("{name}: {version} → {latest}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "chart",
                            source_type: "oci",
                            current: version.clone(),
                            latest: Some(latest),
                            up_to_date,
                        });
                    }
                    Ok(None) => {
                        task.finish_ok(&format!("{name}: no tags"));
                    }
                    Err(e) => {
                        task.finish_err(&format!("{name}: {e}"));
                        entries.push(OutdatedEntry {
                            name: name.clone(),
                            kind: "chart",
                            source_type: "oci",
                            current: version.clone(),
                            latest: None,
                            up_to_date: false,
                        });
                    }
                }
            }
            // file has no version concept
            _ => {}
        }
    }

    Ok(entries)
}

// --- husako update (M19) ---

#[derive(Debug)]
pub struct UpdateOptions {
    pub project_root: PathBuf,
    pub name: Option<String>,
    pub resources_only: bool,
    pub charts_only: bool,
    pub dry_run: bool,
}

#[derive(Debug)]
pub struct UpdatedEntry {
    pub name: String,
    pub kind: &'static str,
    pub old_version: String,
    pub new_version: String,
}

#[derive(Debug)]
pub struct UpdateResult {
    pub updated: Vec<UpdatedEntry>,
    pub skipped: Vec<String>,
    pub failed: Vec<(String, String)>,
}

pub fn update_dependencies(
    options: &UpdateOptions,
    progress: &dyn ProgressReporter,
) -> Result<UpdateResult, HusakoError> {
    let outdated = check_outdated(&options.project_root, progress)?;

    let mut result = UpdateResult {
        updated: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
    };

    // Filter entries
    let filtered: Vec<_> = outdated
        .into_iter()
        .filter(|e| {
            if let Some(ref target) = options.name {
                return &e.name == target;
            }
            if options.resources_only && e.kind != "resource" {
                return false;
            }
            if options.charts_only && e.kind != "chart" {
                return false;
            }
            true
        })
        .collect();

    let mut doc_and_path = None;

    for entry in filtered {
        let Some(ref latest) = entry.latest else {
            result
                .failed
                .push((entry.name, "could not determine latest version".to_string()));
            continue;
        };

        if entry.up_to_date {
            result.skipped.push(entry.name);
            continue;
        }

        if options.dry_run {
            result.updated.push(UpdatedEntry {
                name: entry.name,
                kind: entry.kind,
                old_version: entry.current,
                new_version: latest.clone(),
            });
            continue;
        }

        // Load TOML document lazily
        if doc_and_path.is_none() {
            doc_and_path = Some(husako_config::edit::load_document(&options.project_root)?);
        }
        let (doc, _) = doc_and_path.as_mut().unwrap();

        let updated = if entry.kind == "resource" {
            husako_config::edit::update_resource_version(doc, &entry.name, latest)
        } else {
            husako_config::edit::update_chart_version(doc, &entry.name, latest)
        };

        if updated {
            result.updated.push(UpdatedEntry {
                name: entry.name,
                kind: entry.kind,
                old_version: entry.current,
                new_version: latest.clone(),
            });
        }
    }

    // Save if we modified the document
    if let Some((doc, path)) = &doc_and_path
        && !result.updated.is_empty()
    {
        husako_config::edit::save_document(doc, path)?;
    }

    // Auto-regenerate types if we updated anything
    if !options.dry_run && !result.updated.is_empty() {
        let task = progress.start_task("Regenerating types...");
        let config = husako_config::load(&options.project_root)?;
        let gen_options = GenerateOptions {
            project_root: options.project_root.clone(),
            openapi: None,
            skip_k8s: false,
            config,
        };
        match generate(&gen_options, progress) {
            Ok(()) => task.finish_ok("Types regenerated"),
            Err(e) => task.finish_err(&format!("Type generation failed: {e}")),
        }
    }

    Ok(result)
}

// --- husako info (M20) ---

#[derive(Debug)]
pub struct ProjectSummary {
    pub project_root: PathBuf,
    pub config_valid: bool,
    pub resources: Vec<DependencyInfo>,
    pub charts: Vec<DependencyInfo>,
    pub cache_size: u64,
    pub type_file_count: usize,
    pub types_size: u64,
}

pub fn project_summary(project_root: &Path) -> Result<ProjectSummary, HusakoError> {
    let config = husako_config::load(project_root);
    let config_valid = config.is_ok();

    let deps = list_dependencies(project_root).unwrap_or(DependencyList {
        resources: Vec::new(),
        charts: Vec::new(),
        plugins: Vec::new(),
    });

    let cache_dir = project_root.join(".husako/cache");
    let types_dir = project_root.join(".husako/types");

    let cache_size = if cache_dir.exists() {
        dir_size(&cache_dir)
    } else {
        0
    };

    let (type_file_count, types_size) = if types_dir.exists() {
        count_files_and_size(&types_dir)
    } else {
        (0, 0)
    };

    Ok(ProjectSummary {
        project_root: project_root.to_path_buf(),
        config_valid,
        resources: deps.resources,
        charts: deps.charts,
        cache_size,
        type_file_count,
        types_size,
    })
}

#[derive(Debug)]
pub struct DependencyDetail {
    pub info: DependencyInfo,
    pub cache_path: Option<PathBuf>,
    pub cache_size: u64,
    pub type_files: Vec<(PathBuf, u64)>,
    pub schema_property_count: Option<(usize, usize)>,
    pub group_versions: Vec<(String, Vec<String>)>,
}

pub fn dependency_detail(project_root: &Path, name: &str) -> Result<DependencyDetail, HusakoError> {
    let config = husako_config::load(project_root)?;
    let Some(cfg) = config else {
        return Err(HusakoError::Config(husako_config::ConfigError::Validation(
            "no husako.toml found".to_string(),
        )));
    };

    // Check resources first
    if let Some(source) = cfg.resources.get(name) {
        let info = resource_info(name, source);
        let types_dir = project_root.join(".husako/types/k8s");
        let type_files = list_type_files(&types_dir);

        // Try to read group-versions from generated types
        let group_versions = read_group_versions(&types_dir);

        let (cache_path, cache_size) = resource_cache_info(source, project_root);

        return Ok(DependencyDetail {
            info,
            cache_path,
            cache_size,
            type_files,
            schema_property_count: None,
            group_versions,
        });
    }

    // Check charts
    if let Some(source) = cfg.charts.get(name) {
        let info = chart_info(name, source);
        let types_dir = project_root.join(".husako/types/helm");
        let type_files = list_chart_type_files(&types_dir, name);
        let schema_property_count = read_chart_schema_props(project_root, name);
        let (cache_path, cache_size) = chart_cache_info(source, project_root);

        return Ok(DependencyDetail {
            info,
            cache_path,
            cache_size,
            type_files,
            schema_property_count,
            group_versions: Vec::new(),
        });
    }

    Err(HusakoError::Config(husako_config::ConfigError::Validation(
        format!("dependency '{name}' not found"),
    )))
}

fn count_files_and_size(dir: &Path) -> (usize, u64) {
    let mut count = 0;
    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let meta = entry.metadata();
            if let Ok(m) = meta {
                if m.is_dir() {
                    let (c, s) = count_files_and_size(&entry.path());
                    count += c;
                    size += s;
                } else {
                    count += 1;
                    size += m.len();
                }
            }
        }
    }
    (count, size)
}

fn list_type_files(dir: &Path) -> Vec<(PathBuf, u64)> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && meta.is_file()
            {
                files.push((entry.path(), meta.len()));
            }
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

fn list_chart_type_files(dir: &Path, chart_name: &str) -> Vec<(PathBuf, u64)> {
    let mut files = Vec::new();
    for ext in ["d.ts", "js"] {
        let path = dir.join(format!("{chart_name}.{ext}"));
        if let Ok(meta) = path.metadata() {
            files.push((path, meta.len()));
        }
    }
    files
}

fn read_group_versions(types_dir: &Path) -> Vec<(String, Vec<String>)> {
    let mut gvs: Vec<(String, Vec<String>)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(types_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "ts")
                && path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(".d.ts"))
            {
                let stem = path
                    .file_stem()
                    .unwrap()
                    .to_string_lossy()
                    .trim_end_matches(".d")
                    .to_string();
                let gv = stem.replace("__", "/");
                gvs.push((gv, Vec::new()));
            }
        }
    }
    gvs.sort_by(|a, b| a.0.cmp(&b.0));
    gvs
}

fn resource_cache_info(
    source: &husako_config::SchemaSource,
    project_root: &Path,
) -> (Option<PathBuf>, u64) {
    let cache_base = project_root.join(".husako/cache");
    match source {
        husako_config::SchemaSource::Release { version } => {
            let path = cache_base.join(format!("release/v{version}.0"));
            let size = if path.exists() { dir_size(&path) } else { 0 };
            (Some(path), size)
        }
        _ => (None, 0),
    }
}

fn chart_cache_info(
    _source: &husako_config::ChartSource,
    _project_root: &Path,
) -> (Option<PathBuf>, u64) {
    (None, 0)
}

fn read_chart_schema_props(project_root: &Path, chart_name: &str) -> Option<(usize, usize)> {
    let dts_path = project_root.join(format!(".husako/types/helm/{chart_name}.d.ts"));
    if !dts_path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&dts_path).ok()?;
    // Count properties in ValuesSpec interface
    let total = content.matches("?: ").count() + content.matches(": ").count();
    let top_level = content
        .lines()
        .filter(|l| {
            l.starts_with("  ")
                && !l.starts_with("    ")
                && (l.contains("?: ") || l.contains(": "))
                && !l.contains("export")
                && !l.contains("class")
                && !l.contains("interface")
        })
        .count();
    Some((total, top_level))
}

// --- husako debug (M20) ---

#[derive(Debug)]
pub struct DebugReport {
    pub config_ok: Option<bool>,
    pub types_exist: bool,
    pub type_file_count: usize,
    pub tsconfig_ok: bool,
    pub tsconfig_has_paths: bool,
    pub stale: bool,
    pub cache_size: u64,
    pub issues: Vec<String>,
    pub suggestions: Vec<String>,
}

pub fn debug_project(project_root: &Path) -> Result<DebugReport, HusakoError> {
    let config_path = project_root.join(husako_config::CONFIG_FILENAME);
    let types_dir = project_root.join(".husako/types");
    let cache_dir = project_root.join(".husako/cache");
    let tsconfig_path = project_root.join("tsconfig.json");

    let mut issues = Vec::new();
    let mut suggestions = Vec::new();

    // 1. Check config
    let config_ok = if config_path.exists() {
        match husako_config::load(project_root) {
            Ok(_) => Some(true),
            Err(e) => {
                issues.push(format!("husako.toml parse error: {e}"));
                Some(false)
            }
        }
    } else {
        issues.push("husako.toml not found".to_string());
        suggestions.push("Run 'husako init' to initialize a project".to_string());
        None
    };

    // 2. Check types directory
    let types_exist = types_dir.exists();
    let (type_file_count, _) = if types_exist {
        count_files_and_size(&types_dir)
    } else {
        issues.push(".husako/types/ directory not found".to_string());
        suggestions.push("Run 'husako generate' to create type definitions".to_string());
        (0, 0)
    };

    // 3. Check tsconfig.json
    let (tsconfig_ok, tsconfig_has_paths) = if tsconfig_path.exists() {
        let content = std::fs::read_to_string(&tsconfig_path).unwrap_or_default();
        let stripped = strip_jsonc(&content);
        match serde_json::from_str::<serde_json::Value>(&stripped) {
            Ok(parsed) => {
                let has_husako = parsed.pointer("/compilerOptions/paths/husako").is_some();
                let has_k8s = parsed.pointer("/compilerOptions/paths/k8s~1*").is_some();
                if !has_husako && !has_k8s {
                    issues.push("tsconfig.json is missing husako path mappings".to_string());
                    suggestions.push("Run 'husako generate' to update tsconfig.json".to_string());
                }
                (true, has_husako || has_k8s)
            }
            Err(_) => {
                issues.push("tsconfig.json could not be parsed".to_string());
                (false, false)
            }
        }
    } else {
        issues.push("tsconfig.json not found".to_string());
        suggestions.push("Run 'husako generate' to create tsconfig.json".to_string());
        (false, false)
    };

    // 4. Staleness check
    let stale = if config_path.exists() && types_dir.exists() {
        let config_mtime = config_path.metadata().and_then(|m| m.modified()).ok();
        let types_mtime = types_dir.metadata().and_then(|m| m.modified()).ok();
        match (config_mtime, types_mtime) {
            (Some(c), Some(t)) if c > t => {
                issues
                    .push("Types may be stale (husako.toml newer than .husako/types/)".to_string());
                suggestions.push("Run 'husako generate' to update".to_string());
                true
            }
            _ => false,
        }
    } else {
        false
    };

    // 5. Cache size
    let cache_size = if cache_dir.exists() {
        dir_size(&cache_dir)
    } else {
        0
    };

    Ok(DebugReport {
        config_ok,
        types_exist,
        type_file_count,
        tsconfig_ok,
        tsconfig_has_paths,
        stale,
        cache_size,
        issues,
        suggestions,
    })
}

// --- husako validate (M20) ---

#[derive(Debug)]
pub struct ValidateResult {
    pub resource_count: usize,
    pub validation_errors: Vec<String>,
}

pub fn validate_file(
    source: &str,
    filename: &str,
    options: &RenderOptions,
) -> Result<ValidateResult, HusakoError> {
    let js = husako_compile_oxc::compile(source, filename)?;

    let entry_path = std::path::Path::new(filename)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(filename));

    let generated_types_dir = options
        .project_root
        .join(".husako/types")
        .canonicalize()
        .ok();

    let plugin_modules = load_plugin_modules(&options.project_root);

    let exec_options = ExecuteOptions {
        entry_path,
        project_root: options.project_root.clone(),
        allow_outside_root: options.allow_outside_root,
        timeout_ms: options.timeout_ms,
        max_heap_mb: options.max_heap_mb,
        generated_types_dir,
        plugin_modules,
    };

    let value = husako_runtime_qjs::execute(&js, &exec_options)?;

    let resource_count = if let serde_json::Value::Array(arr) = &value {
        arr.len()
    } else {
        1
    };

    let validation_errors =
        if let Err(errors) = validate::validate(&value, options.schema_store.as_ref()) {
            errors.iter().map(|e| e.to_string()).collect()
        } else {
            Vec::new()
        };

    if !validation_errors.is_empty() {
        return Err(HusakoError::Validation(validation_errors.join("\n")));
    }

    Ok(ValidateResult {
        resource_count,
        validation_errors,
    })
}

/// Strip JSONC features (comments and trailing commas) to produce valid JSON.
///
/// tsconfig.json supports JSONC format: `//` line comments, `/* */` block comments,
/// and trailing commas before `}` or `]`. This function strips those so `serde_json`
/// can parse the result.
fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Inside a string literal — copy verbatim (handles escaped quotes)
        if chars[i] == '"' {
            out.push('"');
            i += 1;
            while i < len {
                if chars[i] == '\\' && i + 1 < len {
                    out.push(chars[i]);
                    out.push(chars[i + 1]);
                    i += 2;
                } else if chars[i] == '"' {
                    out.push('"');
                    i += 1;
                    break;
                } else {
                    out.push(chars[i]);
                    i += 1;
                }
            }
            continue;
        }

        // Line comment
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            while i < len && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comment
        if chars[i] == '/' && i + 1 < len && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < len && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip */
            }
            continue;
        }

        // Trailing comma: comma followed (ignoring whitespace) by } or ]
        if chars[i] == ',' {
            let mut j = i + 1;
            while j < len && chars[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                // Skip the comma, keep whitespace for formatting
                i += 1;
                continue;
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

/// Load plugin module mappings from installed plugins under `.husako/plugins/`.
///
/// Scans each plugin directory for a `plugin.toml` manifest and builds a
/// HashMap of import specifier → absolute `.js` path for the PluginResolver.
fn load_plugin_modules(project_root: &Path) -> std::collections::HashMap<String, PathBuf> {
    let mut modules = std::collections::HashMap::new();
    let plugins_dir = project_root.join(".husako/plugins");
    if !plugins_dir.is_dir() {
        return modules;
    }

    let Ok(entries) = std::fs::read_dir(&plugins_dir) else {
        return modules;
    };

    for entry in entries.flatten() {
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }
        let Ok(manifest) = husako_config::load_plugin_manifest(&plugin_dir) else {
            continue;
        };
        for (specifier, rel_path) in &manifest.modules {
            let abs_path = plugin_dir.join(rel_path);
            modules.insert(specifier.clone(), abs_path);
        }
    }

    modules
}

fn new_tsconfig(husako_paths: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "compilerOptions": {
            "strict": true,
            "module": "ESNext",
            "moduleResolution": "bundler",
            "baseUrl": ".",
            "paths": husako_paths
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_options() -> RenderOptions {
        RenderOptions {
            project_root: PathBuf::from("/tmp"),
            allow_outside_root: false,
            schema_store: None,
            timeout_ms: None,
            max_heap_mb: None,
            verbose: false,
        }
    }

    #[test]
    fn end_to_end_render() {
        let ts = r#"
            import { build } from "husako";
            build([{ _render() { return { apiVersion: "v1", kind: "Namespace", metadata: { name: "test" } }; } }]);
        "#;
        let yaml = render(ts, "test.ts", &test_options()).unwrap();
        assert!(yaml.contains("apiVersion: v1"));
        assert!(yaml.contains("kind: Namespace"));
        assert!(yaml.contains("name: test"));
    }

    #[test]
    fn compile_error_propagates() {
        let ts = "const = ;";
        let err = render(ts, "bad.ts", &test_options()).unwrap_err();
        assert!(matches!(err, HusakoError::Compile(_)));
    }

    #[test]
    fn missing_build_propagates() {
        let ts = r#"import { build } from "husako"; const x = 1;"#;
        let err = render(ts, "test.ts", &test_options()).unwrap_err();
        assert!(matches!(
            err,
            HusakoError::Runtime(husako_runtime_qjs::RuntimeError::BuildNotCalled)
        ));
    }

    #[test]
    fn generate_skip_k8s_writes_static_dts() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        let opts = GenerateOptions {
            project_root: root.clone(),
            openapi: None,
            skip_k8s: true,
            config: None,
        };
        generate(&opts, &progress::SilentProgress).unwrap();

        // Check static .d.ts files exist
        assert!(root.join(".husako/types/husako.d.ts").exists());
        assert!(root.join(".husako/types/husako/_base.d.ts").exists());

        // Check tsconfig.json
        let tsconfig = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&tsconfig).unwrap();
        assert!(parsed["compilerOptions"]["paths"]["husako"].is_array());
        assert!(parsed["compilerOptions"]["paths"]["k8s/*"].is_array());

        // No k8s/ directory
        assert!(!root.join(".husako/types/k8s").exists());
    }

    #[test]
    fn generate_updates_existing_tsconfig() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        // Pre-create tsconfig.json with existing content
        let existing = serde_json::json!({
            "compilerOptions": {
                "strict": true,
                "target": "ES2020",
                "paths": {
                    "mylib/*": ["./lib/*"]
                }
            },
            "include": ["src/**/*"]
        });
        std::fs::write(
            root.join("tsconfig.json"),
            serde_json::to_string_pretty(&existing).unwrap(),
        )
        .unwrap();

        let opts = GenerateOptions {
            project_root: root.clone(),
            openapi: None,
            skip_k8s: true,
            config: None,
        };
        generate(&opts, &progress::SilentProgress).unwrap();

        let tsconfig = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&tsconfig).unwrap();

        // Original fields preserved
        assert_eq!(parsed["compilerOptions"]["target"], "ES2020");
        assert!(parsed["include"].is_array());

        // Original path preserved
        assert!(parsed["compilerOptions"]["paths"]["mylib/*"].is_array());

        // husako paths added
        assert!(parsed["compilerOptions"]["paths"]["husako"].is_array());
        assert!(parsed["compilerOptions"]["paths"]["k8s/*"].is_array());
    }

    #[test]
    fn template_name_from_str() {
        assert_eq!(
            TemplateName::from_str("simple").unwrap(),
            TemplateName::Simple
        );
        assert_eq!(
            TemplateName::from_str("project").unwrap(),
            TemplateName::Project
        );
        assert_eq!(
            TemplateName::from_str("multi-env").unwrap(),
            TemplateName::MultiEnv
        );
        assert!(TemplateName::from_str("unknown").is_err());
    }

    #[test]
    fn template_name_display() {
        assert_eq!(TemplateName::Simple.to_string(), "simple");
        assert_eq!(TemplateName::Project.to_string(), "project");
        assert_eq!(TemplateName::MultiEnv.to_string(), "multi-env");
    }

    #[test]
    fn scaffold_simple_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");

        let opts = ScaffoldOptions {
            directory: dir.clone(),
            template: TemplateName::Simple,
            k8s_version: "1.35".to_string(),
        };
        scaffold(&opts).unwrap();

        assert!(dir.join(".gitignore").exists());
        assert!(dir.join("husako.toml").exists());
        assert!(dir.join("entry.ts").exists());
    }

    #[test]
    fn scaffold_replaces_k8s_version_placeholder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");

        let opts = ScaffoldOptions {
            directory: dir.clone(),
            template: TemplateName::Simple,
            k8s_version: "1.32".to_string(),
        };
        scaffold(&opts).unwrap();

        let config = std::fs::read_to_string(dir.join("husako.toml")).unwrap();
        assert!(config.contains("version = \"1.32\""));
        assert!(!config.contains("%K8S_VERSION%"));
    }

    #[test]
    fn init_replaces_k8s_version_placeholder() {
        let tmp = tempfile::tempdir().unwrap();

        let opts = InitOptions {
            directory: tmp.path().to_path_buf(),
            template: TemplateName::Simple,
            k8s_version: "1.33".to_string(),
        };
        init(&opts).unwrap();

        let config = std::fs::read_to_string(tmp.path().join("husako.toml")).unwrap();
        assert!(config.contains("version = \"1.33\""));
        assert!(!config.contains("%K8S_VERSION%"));
    }

    #[test]
    fn scaffold_project_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");

        let opts = ScaffoldOptions {
            directory: dir.clone(),
            template: TemplateName::Project,
            k8s_version: "1.35".to_string(),
        };
        scaffold(&opts).unwrap();

        assert!(dir.join(".gitignore").exists());
        assert!(dir.join("husako.toml").exists());
        assert!(dir.join("env/dev.ts").exists());
        assert!(dir.join("deployments/nginx.ts").exists());
        assert!(dir.join("lib/index.ts").exists());
        assert!(dir.join("lib/metadata.ts").exists());
    }

    #[test]
    fn scaffold_multi_env_creates_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");

        let opts = ScaffoldOptions {
            directory: dir.clone(),
            template: TemplateName::MultiEnv,
            k8s_version: "1.35".to_string(),
        };
        scaffold(&opts).unwrap();

        assert!(dir.join(".gitignore").exists());
        assert!(dir.join("husako.toml").exists());
        assert!(dir.join("base/nginx.ts").exists());
        assert!(dir.join("base/service.ts").exists());
        assert!(dir.join("dev/main.ts").exists());
        assert!(dir.join("staging/main.ts").exists());
        assert!(dir.join("release/main.ts").exists());
    }

    #[test]
    fn scaffold_rejects_nonempty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("existing.txt"), "content").unwrap();

        let opts = ScaffoldOptions {
            directory: dir,
            template: TemplateName::Simple,
            k8s_version: "1.35".to_string(),
        };
        let err = scaffold(&opts).unwrap_err();
        assert!(matches!(err, HusakoError::GenerateIo(_)));
        assert!(err.to_string().contains("not empty"));
    }

    #[test]
    fn scaffold_allows_empty_existing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-app");
        std::fs::create_dir_all(&dir).unwrap();

        let opts = ScaffoldOptions {
            directory: dir.clone(),
            template: TemplateName::Simple,
            k8s_version: "1.35".to_string(),
        };
        scaffold(&opts).unwrap();

        assert!(dir.join("entry.ts").exists());
    }

    #[test]
    fn generate_chart_types_from_file_source() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        // Create a values.schema.json
        std::fs::write(
            root.join("values.schema.json"),
            r#"{
                "type": "object",
                "properties": {
                    "replicaCount": { "type": "integer" },
                    "image": {
                        "type": "object",
                        "properties": {
                            "repository": { "type": "string" },
                            "tag": { "type": "string" }
                        }
                    }
                }
            }"#,
        )
        .unwrap();

        let config = husako_config::HusakoConfig {
            charts: std::collections::HashMap::from([(
                "my-chart".to_string(),
                husako_config::ChartSource::File {
                    path: "values.schema.json".to_string(),
                },
            )]),
            ..Default::default()
        };

        let opts = GenerateOptions {
            project_root: root.clone(),
            openapi: None,
            skip_k8s: true,
            config: Some(config),
        };
        generate(&opts, &progress::SilentProgress).unwrap();

        // Check chart type files exist
        assert!(root.join(".husako/types/helm/my-chart.d.ts").exists());
        assert!(root.join(".husako/types/helm/my-chart.js").exists());

        // Check DTS content
        let dts = std::fs::read_to_string(root.join(".husako/types/helm/my-chart.d.ts")).unwrap();
        assert!(dts.contains("export interface MyChartSpec"));
        assert!(dts.contains("replicaCount"));
        assert!(dts.contains("export interface MyChart extends _SchemaBuilder"));
        assert!(dts.contains("export function MyChart(): MyChart;"));

        // Check JS content
        let js = std::fs::read_to_string(root.join(".husako/types/helm/my-chart.js")).unwrap();
        assert!(js.contains("class _MyChart extends _SchemaBuilder"));
        assert!(js.contains("export function MyChart()"));

        // Check tsconfig includes helm path
        let tsconfig = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&tsconfig).unwrap();
        assert!(parsed["compilerOptions"]["paths"]["helm/*"].is_array());
    }

    #[test]
    fn generate_without_charts_no_helm_path() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        let opts = GenerateOptions {
            project_root: root.clone(),
            openapi: None,
            skip_k8s: true,
            config: None,
        };
        generate(&opts, &progress::SilentProgress).unwrap();

        // No helm path in tsconfig when no charts
        let tsconfig = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&tsconfig).unwrap();
        assert!(parsed["compilerOptions"]["paths"]["helm/*"].is_null());
    }

    #[test]
    fn strip_jsonc_line_comments() {
        let input = r#"{
  // This is a comment
  "key": "value" // inline comment
}"#;
        let stripped = strip_jsonc(input);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn strip_jsonc_block_comments() {
        let input = r#"{
  /* block comment */
  "key": "value",
  "other": /* inline block */ "data"
}"#;
        let stripped = strip_jsonc(input);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["key"], "value");
        assert_eq!(parsed["other"], "data");
    }

    #[test]
    fn strip_jsonc_trailing_commas() {
        let input = r#"{
  "a": 1,
  "b": [1, 2, 3,],
}"#;
        let stripped = strip_jsonc(input);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["a"], 1);
        assert_eq!(parsed["b"][2], 3);
    }

    #[test]
    fn strip_jsonc_preserves_strings_with_slashes() {
        let input = r#"{"url": "https://example.com", "path": "a//b"}"#;
        let stripped = strip_jsonc(input);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["url"], "https://example.com");
        assert_eq!(parsed["path"], "a//b");
    }

    #[test]
    fn strip_jsonc_tsc_init_style() {
        // Simulates the style of tsconfig.json produced by `tsc --init`
        let input = r#"{
  "compilerOptions": {
    /* Visit https://aka.ms/tsconfig to read more */
    "target": "es2016",
    // "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
  }
}"#;
        let stripped = strip_jsonc(input);
        let parsed: serde_json::Value = serde_json::from_str(&stripped).unwrap();
        assert_eq!(parsed["compilerOptions"]["target"], "es2016");
        assert_eq!(parsed["compilerOptions"]["strict"], true);
        // Commented-out module should not appear
        assert!(parsed["compilerOptions"]["module"].is_null());
    }

    #[test]
    fn generate_updates_jsonc_tsconfig() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();

        // Pre-create tsconfig.json with JSONC features (comments + trailing comma)
        std::fs::write(
            root.join("tsconfig.json"),
            r#"{
  "compilerOptions": {
    // TypeScript options
    "strict": true,
    "target": "ES2022",
  }
}"#,
        )
        .unwrap();

        let opts = GenerateOptions {
            project_root: root.clone(),
            openapi: None,
            skip_k8s: true,
            config: None,
        };
        generate(&opts, &progress::SilentProgress).unwrap();

        let tsconfig = std::fs::read_to_string(root.join("tsconfig.json")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&tsconfig).unwrap();

        // Original fields preserved
        assert_eq!(parsed["compilerOptions"]["target"], "ES2022");
        assert_eq!(parsed["compilerOptions"]["strict"], true);

        // husako paths added
        assert!(parsed["compilerOptions"]["paths"]["husako"].is_array());
        assert!(parsed["compilerOptions"]["paths"]["k8s/*"].is_array());
    }

    // --- M16 tests: init, clean, list ---

    #[test]
    fn init_simple_template() {
        let tmp = tempfile::tempdir().unwrap();

        let opts = InitOptions {
            directory: tmp.path().to_path_buf(),
            template: TemplateName::Simple,
            k8s_version: "1.35".to_string(),
        };
        init(&opts).unwrap();

        assert!(tmp.path().join("husako.toml").exists());
        assert!(tmp.path().join("entry.ts").exists());
        assert!(tmp.path().join(".gitignore").exists());
    }

    #[test]
    fn init_project_template() {
        let tmp = tempfile::tempdir().unwrap();

        let opts = InitOptions {
            directory: tmp.path().to_path_buf(),
            template: TemplateName::Project,
            k8s_version: "1.35".to_string(),
        };
        init(&opts).unwrap();

        assert!(tmp.path().join("husako.toml").exists());
        assert!(tmp.path().join("env/dev.ts").exists());
    }

    #[test]
    fn init_error_if_config_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "").unwrap();

        let opts = InitOptions {
            directory: tmp.path().to_path_buf(),
            template: TemplateName::Simple,
            k8s_version: "1.35".to_string(),
        };
        let err = init(&opts).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn init_works_in_nonempty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("existing.txt"), "content").unwrap();

        let opts = InitOptions {
            directory: tmp.path().to_path_buf(),
            template: TemplateName::Simple,
            k8s_version: "1.35".to_string(),
        };
        init(&opts).unwrap();

        assert!(tmp.path().join("husako.toml").exists());
        assert!(tmp.path().join("existing.txt").exists());
    }

    #[test]
    fn init_appends_gitignore() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".gitignore"), "node_modules/\n").unwrap();

        let opts = InitOptions {
            directory: tmp.path().to_path_buf(),
            template: TemplateName::Simple,
            k8s_version: "1.35".to_string(),
        };
        init(&opts).unwrap();

        let content = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        assert!(content.contains("node_modules/"));
        assert!(content.contains(".husako/"));
    }

    #[test]
    fn init_skips_gitignore_if_husako_present() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".gitignore"), ".husako/\n").unwrap();

        let opts = InitOptions {
            directory: tmp.path().to_path_buf(),
            template: TemplateName::Simple,
            k8s_version: "1.35".to_string(),
        };
        init(&opts).unwrap();

        let content = std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap();
        // Should not have duplicate .husako/ lines
        assert_eq!(content.matches(".husako/").count(), 1);
    }

    #[test]
    fn clean_cache_only() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".husako/cache")).unwrap();
        std::fs::write(root.join(".husako/cache/test.json"), "data").unwrap();
        std::fs::create_dir_all(root.join(".husako/types")).unwrap();
        std::fs::write(root.join(".husako/types/test.d.ts"), "types").unwrap();

        let opts = CleanOptions {
            project_root: root.to_path_buf(),
            cache: true,
            types: false,
        };
        let result = clean(&opts).unwrap();
        assert!(result.cache_removed);
        assert!(!result.types_removed);
        assert!(!root.join(".husako/cache").exists());
        assert!(root.join(".husako/types").exists());
    }

    #[test]
    fn clean_types_only() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".husako/cache")).unwrap();
        std::fs::create_dir_all(root.join(".husako/types")).unwrap();
        std::fs::write(root.join(".husako/types/test.d.ts"), "types").unwrap();

        let opts = CleanOptions {
            project_root: root.to_path_buf(),
            cache: false,
            types: true,
        };
        let result = clean(&opts).unwrap();
        assert!(!result.cache_removed);
        assert!(result.types_removed);
        assert!(root.join(".husako/cache").exists());
        assert!(!root.join(".husako/types").exists());
    }

    #[test]
    fn clean_both() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".husako/cache")).unwrap();
        std::fs::create_dir_all(root.join(".husako/types")).unwrap();
        // Simulate leftover plugins directory (from a previous plugin remove)
        std::fs::create_dir_all(root.join(".husako/plugins")).unwrap();

        let opts = CleanOptions {
            project_root: root.to_path_buf(),
            cache: true,
            types: true,
        };
        let result = clean(&opts).unwrap();
        assert!(result.cache_removed);
        assert!(result.types_removed);
        // Entire .husako/ must be gone, not just the subdirs
        assert!(!root.join(".husako").exists());
    }

    #[test]
    fn clean_nothing_exists() {
        let tmp = tempfile::tempdir().unwrap();

        let opts = CleanOptions {
            project_root: tmp.path().to_path_buf(),
            cache: true,
            types: true,
        };
        let result = clean(&opts).unwrap();
        assert!(!result.cache_removed);
        assert!(!result.types_removed);
    }

    #[test]
    fn list_empty_config() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "").unwrap();

        let deps = list_dependencies(tmp.path()).unwrap();
        assert!(deps.resources.is_empty());
        assert!(deps.charts.is_empty());
    }

    #[test]
    fn list_resources_only() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("husako.toml"),
            "[resources]\nkubernetes = { source = \"release\", version = \"1.35\" }\n",
        )
        .unwrap();

        let deps = list_dependencies(tmp.path()).unwrap();
        assert_eq!(deps.resources.len(), 1);
        assert_eq!(deps.resources[0].name, "kubernetes");
        assert_eq!(deps.resources[0].source_type, "release");
        assert_eq!(deps.resources[0].version.as_deref(), Some("1.35"));
        assert!(deps.charts.is_empty());
    }

    #[test]
    fn list_charts_only() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("husako.toml"),
            "[charts]\nmy-chart = { source = \"file\", path = \"./values.schema.json\" }\n",
        )
        .unwrap();

        let deps = list_dependencies(tmp.path()).unwrap();
        assert!(deps.resources.is_empty());
        assert_eq!(deps.charts.len(), 1);
        assert_eq!(deps.charts[0].name, "my-chart");
    }

    #[test]
    fn list_mixed() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("husako.toml"),
            "[resources]\nkubernetes = { source = \"release\", version = \"1.35\" }\n\n[charts]\nmy-chart = { source = \"file\", path = \"./values.schema.json\" }\n",
        )
        .unwrap();

        let deps = list_dependencies(tmp.path()).unwrap();
        assert_eq!(deps.resources.len(), 1);
        assert_eq!(deps.charts.len(), 1);
    }

    #[test]
    fn list_no_config() {
        let tmp = tempfile::tempdir().unwrap();

        let deps = list_dependencies(tmp.path()).unwrap();
        assert!(deps.resources.is_empty());
        assert!(deps.charts.is_empty());
    }

    // --- M17 tests: add, remove ---

    #[test]
    fn add_resource_creates_entry() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "").unwrap();

        let target = AddTarget::Resource {
            name: "kubernetes".to_string(),
            source: husako_config::SchemaSource::Release {
                version: "1.35".to_string(),
            },
        };
        add_dependency(tmp.path(), &target).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("husako.toml")).unwrap();
        assert!(content.contains("kubernetes"));
        assert!(content.contains("release"));
        assert!(content.contains("1.35"));
    }

    #[test]
    fn add_chart_creates_entry() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "").unwrap();

        let target = AddTarget::Chart {
            name: "ingress-nginx".to_string(),
            source: husako_config::ChartSource::Registry {
                repo: "https://kubernetes.github.io/ingress-nginx".to_string(),
                chart: "ingress-nginx".to_string(),
                version: "4.12.0".to_string(),
            },
        };
        add_dependency(tmp.path(), &target).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("husako.toml")).unwrap();
        assert!(content.contains("ingress-nginx"));
        assert!(content.contains("4.12.0"));
    }

    #[test]
    fn remove_resource_from_config() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("husako.toml"),
            "[resources]\nkubernetes = { source = \"release\", version = \"1.35\" }\n",
        )
        .unwrap();

        let result = remove_dependency(tmp.path(), "kubernetes").unwrap();
        assert_eq!(result.section, "resources");

        let content = std::fs::read_to_string(tmp.path().join("husako.toml")).unwrap();
        assert!(!content.contains("kubernetes"));
    }

    #[test]
    fn remove_chart_from_config() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("husako.toml"),
            "[charts]\nmy-chart = { source = \"file\", path = \"./values.schema.json\" }\n",
        )
        .unwrap();

        let result = remove_dependency(tmp.path(), "my-chart").unwrap();
        assert_eq!(result.section, "charts");
    }

    #[test]
    fn remove_nonexistent_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "").unwrap();

        let err = remove_dependency(tmp.path(), "nonexistent").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    // --- M20 tests: info, debug, validate ---

    #[test]
    fn project_summary_empty() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "").unwrap();

        let summary = project_summary(tmp.path()).unwrap();
        assert!(summary.config_valid);
        assert!(summary.resources.is_empty());
        assert!(summary.charts.is_empty());
    }

    #[test]
    fn project_summary_with_deps() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("husako.toml"),
            "[resources]\nkubernetes = { source = \"release\", version = \"1.35\" }\n",
        )
        .unwrap();

        let summary = project_summary(tmp.path()).unwrap();
        assert_eq!(summary.resources.len(), 1);
    }

    #[test]
    fn debug_missing_config() {
        let tmp = tempfile::tempdir().unwrap();

        let report = debug_project(tmp.path()).unwrap();
        assert!(report.config_ok.is_none());
        assert!(!report.types_exist);
        assert!(!report.suggestions.is_empty());
    }

    #[test]
    fn debug_valid_project() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "").unwrap();
        std::fs::create_dir_all(tmp.path().join(".husako/types")).unwrap();
        std::fs::write(tmp.path().join(".husako/types/husako.d.ts"), "").unwrap();

        let opts = GenerateOptions {
            project_root: tmp.path().to_path_buf(),
            openapi: None,
            skip_k8s: true,
            config: None,
        };
        generate(&opts, &progress::SilentProgress).unwrap();

        let report = debug_project(tmp.path()).unwrap();
        assert_eq!(report.config_ok, Some(true));
        assert!(report.types_exist);
        assert!(report.tsconfig_ok);
    }

    #[test]
    fn debug_missing_types() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "").unwrap();

        let report = debug_project(tmp.path()).unwrap();
        assert_eq!(report.config_ok, Some(true));
        assert!(!report.types_exist);
    }

    #[test]
    fn validate_valid_ts() {
        let ts = r#"
            import { build } from "husako";
            build([{ _render() { return { apiVersion: "v1", kind: "Namespace", metadata: { name: "test" } }; } }]);
        "#;
        let options = test_options();
        let result = validate_file(ts, "test.ts", &options).unwrap();
        assert_eq!(result.resource_count, 1);
        assert!(result.validation_errors.is_empty());
    }

    #[test]
    fn validate_compile_error() {
        let ts = "const = ;";
        let options = test_options();
        let err = validate_file(ts, "bad.ts", &options).unwrap_err();
        assert!(matches!(err, HusakoError::Compile(_)));
    }

    #[test]
    fn validate_runtime_error() {
        let ts = r#"import { build } from "husako"; const x = 1;"#;
        let err = validate_file(ts, "test.ts", &test_options()).unwrap_err();
        assert!(matches!(err, HusakoError::Runtime(_)));
    }

    #[test]
    fn dependency_detail_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("husako.toml"), "").unwrap();

        let err = dependency_detail(tmp.path(), "nonexistent").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn dependency_detail_resource() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("husako.toml"),
            "[resources]\nkubernetes = { source = \"release\", version = \"1.35\" }\n",
        )
        .unwrap();

        let detail = dependency_detail(tmp.path(), "kubernetes").unwrap();
        assert_eq!(detail.info.name, "kubernetes");
        assert_eq!(detail.info.source_type, "release");
        assert_eq!(detail.info.version.as_deref(), Some("1.35"));
    }

    #[test]
    fn dependency_detail_chart() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("husako.toml"),
            "[charts]\nmy-chart = { source = \"file\", path = \"./values.schema.json\" }\n",
        )
        .unwrap();

        let detail = dependency_detail(tmp.path(), "my-chart").unwrap();
        assert_eq!(detail.info.name, "my-chart");
        assert_eq!(detail.info.source_type, "file");
    }
}
