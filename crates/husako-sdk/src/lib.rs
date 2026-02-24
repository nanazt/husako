pub const HUSAKO_MODULE: &str = include_str!("js/husako.js");
pub const HUSAKO_BASE: &str = include_str!("js/husako_base.js");
pub const HUSAKO_TEST_MODULE: &str = include_str!("js/husako_test.js");

pub const HUSAKO_DTS: &str = include_str!("dts/husako.d.ts");
pub const HUSAKO_BASE_DTS: &str = include_str!("dts/husako_base.d.ts");
pub const HUSAKO_TEST_DTS: &str = include_str!("dts/husako_test.d.ts");

// Template files for `husako new`
pub const TEMPLATE_GITIGNORE: &str = include_str!("templates/gitignore.txt");

pub const TEMPLATE_SIMPLE_CONFIG: &str = include_str!("templates/simple/husako.toml");
pub const TEMPLATE_SIMPLE_ENTRY: &str = include_str!("templates/simple/entry.ts");

pub const TEMPLATE_PROJECT_CONFIG: &str = include_str!("templates/project/husako.toml");
pub const TEMPLATE_PROJECT_ENV_DEV: &str = include_str!("templates/project/env/dev.ts");
pub const TEMPLATE_PROJECT_DEPLOY_NGINX: &str =
    include_str!("templates/project/deployments/nginx.ts");
pub const TEMPLATE_PROJECT_LIB_INDEX: &str = include_str!("templates/project/lib/index.ts");
pub const TEMPLATE_PROJECT_LIB_METADATA: &str = include_str!("templates/project/lib/metadata.ts");

pub const TEMPLATE_MULTI_ENV_CONFIG: &str = include_str!("templates/multi-env/husako.toml");
pub const TEMPLATE_MULTI_ENV_BASE_NGINX: &str = include_str!("templates/multi-env/base/nginx.ts");
pub const TEMPLATE_MULTI_ENV_BASE_SERVICE: &str =
    include_str!("templates/multi-env/base/service.ts");
pub const TEMPLATE_MULTI_ENV_DEV_MAIN: &str = include_str!("templates/multi-env/dev/main.ts");
pub const TEMPLATE_MULTI_ENV_STAGING_MAIN: &str =
    include_str!("templates/multi-env/staging/main.ts");
pub const TEMPLATE_MULTI_ENV_RELEASE_MAIN: &str =
    include_str!("templates/multi-env/release/main.ts");
