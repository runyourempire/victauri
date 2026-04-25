use victauri_core::registry::CommandInfo;

mod fake_tauri_types {
    pub struct AppHandle;
    pub struct State<T>(std::marker::PhantomData<T>);
    pub struct Window;
    pub struct Webview;
}

#[victauri_macros::inspectable(description = "Save an API key")]
fn save_api_key(provider: String, key: String) -> Result<(), String> {
    let _ = (provider, key);
    Ok(())
}

#[victauri_macros::inspectable(description = "Load user data")]
async fn load_user(user_id: u64, include_profile: Option<bool>) -> Result<String, String> {
    let _ = (user_id, include_profile);
    Ok("user".to_string())
}

#[victauri_macros::inspectable]
fn simple_command() -> String {
    "hello".to_string()
}

#[test]
fn inspectable_generates_schema_fn() {
    let info: CommandInfo = save_api_key__schema();
    assert_eq!(info.name, "save_api_key");
    assert_eq!(info.description.as_deref(), Some("Save an API key"));
    assert!(!info.is_async);
    assert_eq!(info.args.len(), 2);
    assert_eq!(info.args[0].name, "provider");
    assert_eq!(info.args[0].type_name, "String");
    assert!(info.args[0].required);
    assert_eq!(info.args[1].name, "key");
    assert!(info.args[1].required);
}

#[test]
fn inspectable_async_fn() {
    let info: CommandInfo = load_user__schema();
    assert_eq!(info.name, "load_user");
    assert!(info.is_async);
    assert_eq!(info.args.len(), 2);
    assert_eq!(info.args[0].name, "user_id");
    assert_eq!(info.args[0].type_name, "u64");
    assert!(info.args[0].required);
    assert_eq!(info.args[1].name, "include_profile");
    assert!(!info.args[1].required);
}

#[test]
fn inspectable_default_description() {
    let info: CommandInfo = simple_command__schema();
    assert_eq!(info.description.as_deref(), Some("simple command"));
    assert_eq!(info.args.len(), 0);
}
