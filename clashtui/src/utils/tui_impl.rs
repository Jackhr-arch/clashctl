use std::{
    fs::{create_dir_all, File},
    io::Error,
    path::{Path, PathBuf},
};

use super::{tui::parse_yaml, utils as Utils, ClashSrvOp, ClashTuiUtil};

impl ClashTuiUtil {
    pub fn create_yaml_with_template(&self, template_name: &String) -> anyhow::Result<()> {
        use std::borrow::Cow;
        use std::collections::HashMap;
        use std::io::{BufRead, BufReader};
        let template_dir = self.clashtui_dir.join("templates");
        let template_path = template_dir.join(template_name);
        let tpl_parsed_yaml = parse_yaml(&template_path)?;
        let mut out_parsed_yaml = Cow::Borrowed(&tpl_parsed_yaml);

        let proxy_url_file =
            File::open(self.clashtui_dir.join("templates/template_proxy_providers"))?;
        let mut proxy_urls = Vec::new();
        BufReader::new(proxy_url_file)
            .lines()
            .map_while(Result::ok)
            .filter(|v| {
                let val = v.trim();
                !(val.is_empty() || val.starts_with('#'))
            })
            .for_each(|v| proxy_urls.push(v));

        // ## proxy-providers
        // e.g. {provider: [provider0, provider1, ...]}
        let mut pp_names: HashMap<String, Vec<String>> = HashMap::new(); // proxy-provider names
        let mut new_proxy_providers = HashMap::new();
        let pp_mapping = if let Some(serde_yaml::Value::Mapping(pp_mapping)) =
            tpl_parsed_yaml.get("proxy-providers")
        {
            pp_mapping
        } else {
            anyhow::bail!("Failed to parse `proxy-providers`");
        };

        for (pp_key, pp_value) in pp_mapping {
            if pp_value.get("tpl_param").is_none() {
                new_proxy_providers.insert(pp_key.clone(), pp_value.clone());
                continue;
            }

            let pp = if let serde_yaml::Value::Mapping(pp) = pp_value {
                pp
            } else {
                anyhow::bail!("Failed to parse `proxy-providers` value");
            };

            let mut new_pp = pp.clone();
            new_pp.remove("tpl_param");

            for (i, url) in proxy_urls.iter().enumerate() {
                // name: e.g. provier0, provider1, ...
                let the_pp_name = format!("{}{}", pp_key.as_str().unwrap(), i);
                pp_names
                    .entry(pp_key.as_str().unwrap().to_string())
                    .or_default()
                    .push(the_pp_name.clone());

                new_pp.insert(
                    serde_yaml::Value::String("url".to_string()),
                    serde_yaml::Value::String(url.clone()),
                );
                new_pp.insert(
                    serde_yaml::Value::String("path".to_string()),
                    serde_yaml::Value::String(format!("proxy-providers/tpl/{}.yaml", the_pp_name)),
                );
                new_proxy_providers.insert(
                    serde_yaml::Value::String(the_pp_name.clone()),
                    serde_yaml::Value::Mapping(new_pp.clone()),
                );
            }
        }
        out_parsed_yaml.to_mut()["proxy-providers"] =
            serde_yaml::to_value(new_proxy_providers).unwrap();

        // ## proxy-groups
        // e.g. {Auto: [Auto-provider0, Auto-provider1, ...], Select: [Select-provider0, ...]}
        let mut pg_names: HashMap<String, Vec<String>> = HashMap::new();
        let mut new_proxy_groups = serde_yaml::Sequence::new();
        let pg_value = if let Some(serde_yaml::Value::Sequence(pg_value)) =
            tpl_parsed_yaml.get("proxy-groups")
        {
            pg_value
        } else {
            anyhow::bail!("Failed to parse `proxy-groups`.");
        };

        for the_pg_value in pg_value {
            if the_pg_value.get("tpl_param").is_none() {
                new_proxy_groups.push(the_pg_value.clone());
                continue;
            }

            let the_pg = if let serde_yaml::Value::Mapping(the_pg) = the_pg_value {
                the_pg
            } else {
                anyhow::bail!("Failed to parse `proxy-groups` value");
            };

            let mut new_pg = the_pg.clone();
            new_pg.remove("tpl_param");

            let provider_keys = if let Some(serde_yaml::Value::Sequence(provider_keys)) =
                the_pg["tpl_param"].get("providers")
            {
                provider_keys
            } else {
                anyhow::bail!("Failed to parse `providers` in `tpl_param`");
            };

            for the_provider_key in provider_keys {
                let the_pk_str = if let serde_yaml::Value::String(the_pk_str) = the_provider_key {
                    the_pk_str
                } else {
                    anyhow::bail!("Failed to parse string in `providers`");
                };

                let names = if let Some(names) = pp_names.get(the_pk_str) {
                    names
                } else {
                    continue;
                };

                let the_pg_name = if let Some(serde_yaml::Value::String(the_pg_name)) =
                    the_pg_value.get("name")
                {
                    the_pg_name
                } else {
                    anyhow::bail!("Failed to parse `name` in `proxy-groups`")
                };

                for n in names {
                    // new_pg_name: e.g. Auto-provider0, Auto-provider1, Select-provider0, ...
                    let new_pg_name = format!("{}-{}", the_pg_name, n); // proxy-group
                                                                        // names

                    pg_names
                        .entry(the_pg_name.clone())
                        .or_default()
                        .push(new_pg_name.clone());

                    new_pg["name"] = serde_yaml::Value::String(new_pg_name.clone());
                    new_pg.insert(
                        serde_yaml::Value::String("use".to_string()),
                        serde_yaml::Value::Sequence(vec![serde_yaml::Value::String(n.clone())]),
                    );

                    new_proxy_groups.push(serde_yaml::Value::Mapping(new_pg.clone()));
                }
            }
        }
        out_parsed_yaml.to_mut()["proxy-groups"] = serde_yaml::Value::Sequence(new_proxy_groups);

        // ### replace special keys in group-providers
        // e.g. <provider> => provider0, provider1
        // e.g. <Auto> => Auto-provider0, Auto-provider1
        // e.g. <Select> => Select-provider0, Select-provider1
        let pg_sequence = if let Some(serde_yaml::Value::Sequence(pg_sequence)) =
            out_parsed_yaml.to_mut().get_mut("proxy-groups")
        {
            pg_sequence
        } else {
            anyhow::bail!("Failed to parse `proxy-groups`");
        };

        for the_pg_seq in pg_sequence {
            if let Some(providers) = the_pg_seq.get("use") {
                let mut new_providers = Vec::new();
                for p in providers.as_sequence().unwrap() {
                    let p_str = p.as_str().unwrap();
                    if p_str.starts_with('<') && p_str.ends_with('>') {
                        let trimmed_p_str = p_str.trim_matches(|c| c == '<' || c == '>');
                        let provider_names = pp_names.get(trimmed_p_str).unwrap();
                        new_providers.extend(provider_names.iter().cloned());
                    } else {
                        new_providers.push(p_str.to_string());
                    }
                }
                the_pg_seq["use"] = serde_yaml::Value::Sequence(
                    new_providers
                        .into_iter()
                        .map(serde_yaml::Value::String)
                        .collect(),
                );
            }

            if let Some(serde_yaml::Value::Sequence(groups)) = the_pg_seq.get("proxies") {
                let mut new_groups = Vec::new();
                for g in groups {
                    let g_str = g.as_str().unwrap();
                    if g_str.starts_with('<') && g_str.ends_with('>') {
                        let trimmed_g_str = g_str.trim_matches(|c| c == '<' || c == '>');
                        let group_names = pg_names.get(trimmed_g_str).unwrap();
                        new_groups.extend(group_names.iter().cloned());
                    } else {
                        new_groups.push(g_str.to_string());
                    }
                }
                the_pg_seq["proxies"] = serde_yaml::Value::Sequence(
                    new_groups
                        .into_iter()
                        .map(serde_yaml::Value::String)
                        .collect(),
                );
            }
        }

        log::error!("testssdfs");
        let out_yaml_path = self.profile_dir.join(template_name);
        let out_yaml_file = File::create(out_yaml_path)?;
        serde_yaml::to_writer(out_yaml_file, &out_parsed_yaml)?;

        Ok(())
    }

    pub fn get_profile_names(&self) -> Result<Vec<String>, Error> {
        Utils::get_file_names(self.profile_dir.as_path()).map(|mut v| {
            v.sort();
            v
        })
    }
    pub fn get_template_names(&self) -> Result<Vec<String>, Error> {
        Utils::get_file_names(self.clashtui_dir.join("templates").as_path()).map(|mut v| {
            v.sort();
            v
        })
    }
    pub fn get_profile_yaml_path(&self, profile_name: &String) -> PathBuf {
        let profile_path = self.profile_dir.join(profile_name);

        if self.is_profile_yaml(profile_name) {
            profile_path
        } else {
            let profile_cache_dir = self.clashtui_dir.join("profile_cache");
            let profile_yaml_name = Path::new(profile_name).with_extension("yaml");
            profile_cache_dir.join(profile_yaml_name)
        }
    }

    pub fn update_local_profile(
        &self,
        profile_name: &String,
        does_update_all: bool,
    ) -> anyhow::Result<(Vec<(String, String)>, Vec<(String, String)>)> {
        use std::io::Read;
        let net_res_keys = if !does_update_all {
            vec!["proxy-providers"]
        } else {
            vec!["proxy-providers", "rule-providers"]
        };

        let profile_path = self.profile_dir.join(profile_name);
        let mut profile_yaml_path = profile_path.clone();
        let mut net_res: Vec<(String, String)> = Vec::new();
        // ## 如果是订阅链接
        if !self.is_profile_yaml(profile_name) {
            let mut file = File::open(profile_path)?;
            let mut file_content = String::new();
            file.read_to_string(&mut file_content)?;

            let sub_url = file_content.trim();
            let response = self.dl_remote_profile(sub_url)?;

            profile_yaml_path = self.get_profile_yaml_path(profile_name);
            let directory = profile_yaml_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Invalid file path"))?;
            if !directory.exists() {
                create_dir_all(directory)?;
            }
            let mut output_file = File::create(&profile_yaml_path)?;
            response.copy_to(&mut output_file)?;

            net_res.push((
                sub_url.to_string(),
                profile_yaml_path.to_string_lossy().to_string(),
            ))
        }

        // ## 更新 yaml 的网络资源
        let mut file = File::open(profile_yaml_path)?;
        let mut yaml_content = String::new();
        file.read_to_string(&mut yaml_content)?;

        if let Ok(parsed_yaml) = serde_yaml::from_str::<serde_yaml::Value>(yaml_content.as_str()) {
            for key in &net_res_keys {
                let providers =
                    if let Some(serde_yaml::Value::Mapping(providers)) = parsed_yaml.get(key) {
                        providers
                    } else {
                        continue;
                    };

                for (_, provider_value) in providers {
                    let provider_content =
                        if let serde_yaml::Value::Mapping(provider_content) = provider_value {
                            provider_content
                        } else {
                            continue;
                        };

                    if let Some(serde_yaml::Value::String(t)) = provider_content.get("type") {
                        if t != "http" {
                            continue;
                        }
                    } else {
                        continue;
                    };

                    if let (
                        Some(serde_yaml::Value::String(url)),
                        Some(serde_yaml::Value::String(path)),
                    ) = (provider_content.get("url"), provider_content.get("path"))
                    {
                        net_res.push((url.clone(), path.clone()))
                    }
                }
            }
        } else {
            return Err(anyhow::anyhow!("Not yaml?"));
        }
        let mut updated_res = vec![];
        let mut not_updated_res = vec![];
        for (url, path) in &net_res {
            match self.download_file(
                url,
                &Path::new(&self.tui_cfg.clash_cfg_dir).join(path),
            ) {
                Ok(_) => {
                    updated_res.push((url.clone(), path.clone()));
                }
                Err(err) => {
                    not_updated_res.push((url.clone(), path.clone()));
                    log::error!("Failed to Download file: {}", err);
                }
            }
        }

        Ok((updated_res, not_updated_res))
    }

    fn download_file(&self, url: &str, path: &PathBuf) -> Result<(), Error> {
        let response = self.dl_remote_profile(url)?;

        let directory = path
            .parent()
            .ok_or_else(|| Error::new(std::io::ErrorKind::NotFound, "Invalid file path"))?;
        if !directory.exists() {
            create_dir_all(directory)?;
        }

        let mut output_file = File::create(path)?;
        response.copy_to(&mut output_file)?;
        Ok(())
    }
    /// Judging by format
    pub fn is_profile_yaml(&self, profile_name: &String) -> bool {
        let profile_path = self.profile_dir.join(profile_name);
        Self::is_yaml(&profile_path)
    }
    /// Judging by format
    fn is_yaml(path: &Path) -> bool {
        if let Ok(f) = std::fs::File::open(path) {
            serde_yaml::from_reader::<std::fs::File, serde_yaml::Value>(f).is_ok()
        } else {
            false
        }
    }
}
use super::ipc::exec_ipc;
// IPC Related
impl ClashTuiUtil {
    /// Exec `cmd` for given `path`
    ///
    /// Auto detect `cmd` is_empty and use system default app to open `path`
    fn spawn_open(cmd: &String, path: &Path) -> Result<(), Error> {
        use super::ipc::spawn;
        if !cmd.is_empty() {
            let opendir_cmd_with_path = cmd.replace("%s", path.to_str().unwrap_or(""));
            #[cfg(target_os = "windows")]
            return spawn("cmd", vec!["/C", opendir_cmd_with_path.as_str()]);
            #[cfg(target_os = "linux")]
            spawn("sh", vec!["-c", opendir_cmd_with_path.as_str()])
        } else {
            #[cfg(target_os = "windows")]
            return spawn("cmd", vec!["/C", "start", path.to_str().unwrap_or("")]);
            #[cfg(target_os = "linux")]
            spawn("xdg-open", vec![path.to_str().unwrap_or("")])
        }
    }

    pub fn edit_file(&self, path: &Path) -> Result<(), Error> {
        Self::spawn_open(&self.tui_cfg.edit_cmd, path)
    }
    pub fn open_dir(&self, path: &Path) -> Result<(), Error> {
        Self::spawn_open(&self.tui_cfg.open_dir_cmd, path)
    }
    pub fn test_profile_config(&self, path: &str, geodata_mode: bool) -> Result<String, Error> {
        let cmd = format!(
            "{} {} -d {} -f {} -t",
            self.tui_cfg.clash_core_path,
            if geodata_mode { "-m" } else { "" },
            self.tui_cfg.clash_cfg_dir,
            path,
        );
        #[cfg(target_os = "windows")]
        return exec_ipc("cmd", vec!["/C", cmd.as_str()]);
        #[cfg(target_os = "linux")]
        exec_ipc("sh", vec!["-c", cmd.as_str()])
    }

    #[cfg(target_os = "linux")]
    pub fn clash_srv_ctl(&self, op: ClashSrvOp) -> Result<String, Error> {
        match op {
            ClashSrvOp::StartClashService => exec_ipc(
                "systemctl",
                vec!["restart", self.tui_cfg.clash_srv_name.as_str()],
            ),
            ClashSrvOp::StopClashService => exec_ipc(
                "systemctl",
                vec!["stop", self.tui_cfg.clash_srv_name.as_str()],
            ),
            ClashSrvOp::TestClashConfig => {
                self.test_profile_config(self.tui_cfg.clash_cfg_path.as_str(), false)
            }
            ClashSrvOp::SetPermission => exec_ipc(
                "setcap",
                vec![
                    "'cap_net_admin,cap_net_bind_service=+ep'",
                    self.tui_cfg.clash_core_path.as_str(),
                ],
            ),
            _ => Err(Error::new(
                std::io::ErrorKind::NotFound,
                "No Support Adction",
            )),
        }
    }
    #[cfg(target_os = "windows")]
    pub fn clash_srv_ctl(&self, op: ClashSrvOp) -> Result<String, Error> {
        let exe_dir = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        //let nssm_path = exe_dir.join("nssm");
        //let nssm_path_str = nssm_path.to_str().unwrap();
        let nssm_path_str = "nssm";

        let output = match op {
            ClashSrvOp::StartClashService => {
                Self::start_process_as_admin(
                    nssm_path_str,
                    format!("restart {}", self.clash_srv_name).as_str(),
                    true,
                )?;

                Command::new(nssm_path_str)
                    .arg("status")
                    .arg(self.clash_srv_name.as_str())
                    .output()?
            }

            ClashSrvOp::StopClashService => {
                Self::start_process_as_admin(
                    nssm_path_str,
                    &format!("stop {}", self.clash_srv_name),
                    true,
                )?;

                Command::new(nssm_path_str)
                    .arg("status")
                    .arg(self.clash_srv_name.as_str())
                    .output()?
            }

            ClashSrvOp::TestClashConfig => {
                return self.test_profile_config(&self.clash_cfg_path, false);
            }

            ClashSrvOp::InstallSrv => {
                Self::start_process_as_admin(
                    nssm_path_str,
                    &format!(
                        "install {} \"{}\" -d \"{}\" -f \"{}\"",
                        self.clash_srv_name,
                        self.clash_core_path.to_str().unwrap(),
                        self.clash_cfg_dir.to_str().unwrap(),
                        self.clash_cfg_path.to_str().unwrap()
                    ),
                    true,
                )?;

                Command::new(nssm_path_str)
                    .arg("status")
                    .arg(self.clash_srv_name.as_str())
                    .output()?
            }

            ClashSrvOp::UnInstallSrv => Self::execute_powershell_script_as_admin(
                &format!(
                    "{0} stop {1}; {0} remove {1}",
                    nssm_path_str, self.clash_srv_name
                ),
                true,
            )?,

            ClashSrvOp::EnableLoopback => {
                let exe_dir = std::env::current_exe()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .to_path_buf();
                Self::start_process_as_admin(
                    exe_dir.join("EnableLoopback").to_str().unwrap(),
                    "",
                    false,
                )?
            }
            _ => {
                bail!("Do nothing for the ClashTuiOp.")
            }
        };

        Self::string_process_output(output)
    }
}