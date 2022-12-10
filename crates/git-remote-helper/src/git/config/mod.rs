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
