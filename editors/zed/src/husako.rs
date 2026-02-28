use zed_extension_api::{self as zed, LanguageServerId, Result};

struct HusakoExtension;

impl zed::Extension for HusakoExtension {
    fn new() -> Self {
        HusakoExtension
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        match language_server_id.as_ref() {
            "husako-lsp" => {
                let binary = worktree
                    .which("husako")
                    .ok_or("husako not found in PATH. Install husako and ensure it is on your PATH.")?;
                Ok(zed::Command {
                    command: binary,
                    args: vec!["lsp".into()],
                    env: Default::default(),
                })
            }
            "typescript-language-server" => {
                let binary = worktree
                    .which("typescript-language-server")
                    .ok_or("typescript-language-server not found. Install with: npm install -g typescript-language-server typescript")?;
                Ok(zed::Command {
                    command: binary,
                    args: vec!["--stdio".into()],
                    env: Default::default(),
                })
            }
            id => Err(format!("unknown language server: {id}")),
        }
    }
}

zed::register_extension!(HusakoExtension);
