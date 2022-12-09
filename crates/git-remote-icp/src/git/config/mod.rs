use ic_agent::export::Principal;

// TODO: figure out why this doesn't find the setting when used with `git -c`
// let private_key_path = config.string("icp.privateKey").ok_or_else(|| {
//     anyhow!("failed to read icp.privateKey from git config. Set with `git config --global icp.privateKey <path to private key>`")
// })?;

pub fn get(key: &str) -> anyhow::Result<String> {
    let config_value = std::process::Command::new("git")
        .arg("config")
        .arg(key)
        .output()?;

    let config_value = config_value.stdout;
    let config_value = String::from_utf8(config_value)?;
    let config_value = config_value.trim().to_string();

    Ok(config_value)
}

const CANISTER_ID_KEY: &str = "icp.canisterId";
const DEFAULT_CANISTER_ID: &str = "w7uni-tiaaa-aaaam-qaydq-cai";

pub fn canister_id() -> anyhow::Result<Principal> {
    let canister_id = get(REPLICA_URL_KEY).unwrap_or_else(|_| DEFAULT_CANISTER_ID.to_string());
    let principal = Principal::from_text(canister_id)?;
    Ok(principal)
}

const FETCH_ROOT_KEY_KEY: &str = "icp.fetchRootKey";
const DEFAULT_FETCH_ROOT_KEY: bool = false;

pub fn fetch_root_key() -> bool {
    get(FETCH_ROOT_KEY_KEY)
        .map(|config_value| match config_value.as_str() {
            "true" => true,
            _ => false,
        })
        .unwrap_or(DEFAULT_FETCH_ROOT_KEY)
}

const PRIVATE_KEY_KEY: &str = "icp.privateKey";

pub fn private_key() -> anyhow::Result<String> {
    get(PRIVATE_KEY_KEY)
}

const REPLICA_URL_KEY: &str = "icp.replicaUrl";
const DEFAULT_REPLICA_URL: &str = "https://ic0.app";

pub fn replica_url() -> String {
    get(REPLICA_URL_KEY).unwrap_or_else(|_| DEFAULT_REPLICA_URL.to_string())
}
