use zed_extension_api::{self as zed, LanguageServerId, Result};

struct HusakoExtension;

impl zed::Extension for HusakoExtension {
    fn new() -> Self {
        HusakoExtension
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        match language_server_id.as_ref() {
            "husako-lsp" => Ok(zed::Command {
                command: "husako".into(),
                args: vec!["lsp".into()],
                env: Default::default(),
            }),
            "typescript-language-server" => Ok(zed::Command {
                command: "typescript-language-server".into(),
                args: vec!["--stdio".into()],
                env: Default::default(),
            }),
            id => Err(format!("unknown language server: {id}")),
        }
    }
}

zed::register_extension!(HusakoExtension);
