use anyhow::anyhow;
use git_remote_helper::git;
use ic_agent::export::Principal;

const CANISTER_ID_KEY: &str = "icp.canisterId";
const DEFAULT_CANISTER_ID: &str = "w7uni-tiaaa-aaaam-qaydq-cai";

pub fn canister_id() -> anyhow::Result<Principal> {
    let canister_id =
        git::config::get(CANISTER_ID_KEY).unwrap_or_else(|_| DEFAULT_CANISTER_ID.to_string());
    let principal = Principal::from_text(canister_id)?;
    Ok(principal)
}

const FETCH_ROOT_KEY_KEY: &str = "icp.fetchRootKey";
const DEFAULT_FETCH_ROOT_KEY: bool = false;

pub fn fetch_root_key() -> bool {
    git::config::get(FETCH_ROOT_KEY_KEY)
        .map(|config_value| matches!(config_value.as_str(), "true"))
        .unwrap_or(DEFAULT_FETCH_ROOT_KEY)
}

const PRIVATE_KEY_KEY: &str = "icp.privateKey";

pub fn private_key() -> anyhow::Result<String> {
    let private_key_path = git::config::get(PRIVATE_KEY_KEY).map_err(|_| {
        anyhow!("failed to read icp.privateKey from git config. Set `icp.privateKey = <path to private key>`")
    })?;

    let trimmed = private_key_path.trim();

    if trimmed.is_empty() {
        Err(anyhow!("icp.privateKey is empty"))
    } else {
        Ok(trimmed.to_string())
    }
}

const REPLICA_URL_KEY: &str = "icp.replicaUrl";
const DEFAULT_REPLICA_URL: &str = "https://ic0.app";

pub fn replica_url() -> String {
    git::config::get(REPLICA_URL_KEY).unwrap_or_else(|_| DEFAULT_REPLICA_URL.to_string())
}
