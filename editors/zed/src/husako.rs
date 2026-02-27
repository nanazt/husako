use zed_extension_api::{self as zed, LanguageServerId, Result};

struct HusakoExtension;

impl zed::Extension for HusakoExtension {
    fn new() -> Self {
        HusakoExtension
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        Ok(zed::Command {
            command: "husako".into(),
            args: vec!["lsp".into()],
            env: Default::default(),
        })
    }
}

zed::register_extension!(HusakoExtension);
