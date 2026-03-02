use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use crate::utils;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepsConfig {
    pub version: Option<String>,
    pub vars: Option<HashMap<String, String>>,
    pub repos: Option<HashMap<String, Vec<RepoItem>>>,
    pub files: Option<HashMap<String, Vec<FileItem>>>,
    pub actions: Option<HashMap<String, Vec<ActionItem>>>,
    pub linkfiles: Option<HashMap<String, Vec<LinkFileItem>>>,
    pub copyfiles: Option<HashMap<String, Vec<CopyFileItem>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoItem {
    pub url: String,
    pub commit: String,
    pub dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileItem {
    pub url: String,
    pub dir: String,
    #[serde(default)]
    #[serde(deserialize_with = "deserialize_unzip")]
    pub unzip: bool,
    pub multipart: Option<Vec<String>>,
    pub timeout: Option<u64>,
}

fn deserialize_unzip<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Unzip {
        Bool(bool),
        String(String),
    }
    
    match Unzip::deserialize(deserializer)? {
        Unzip::Bool(b) => Ok(b),
        Unzip::String(s) => Ok(s == "true"),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionItem {
    pub command: String,
    pub dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkFileItem {
    pub src: String,
    pub dest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyFileItem {
    pub src: String,
    pub dest: String,
}

#[derive(Debug, Clone)]
pub struct ParsedConfig {
    #[allow(dead_code)]
    pub version: String,
    pub repos: Vec<ParsedRepoItem>,
    pub files: Vec<ParsedFileItem>,
    pub actions: Vec<ParsedActionItem>,
    pub linkfiles: Vec<ParsedLinkFileItem>,
    pub copyfiles: Vec<ParsedCopyFileItem>,
}

#[derive(Debug, Clone)]
pub struct ParsedRepoItem {
    pub url: String,
    pub commit: String,
    pub dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ParsedFileItem {
    pub url: String,
    pub dir: PathBuf,
    pub hash: String,
    pub hash_file: PathBuf,
    pub unzip: bool,
    pub multipart: Option<Vec<String>>,
    pub timeout: u64,
}

#[derive(Debug, Clone)]
pub struct ParsedActionItem {
    pub command: String,
    pub dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ParsedLinkFileItem {
    pub src: PathBuf,
    pub dest: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ParsedCopyFileItem {
    pub src: PathBuf,
    pub dest: PathBuf,
}

#[derive(Debug, Clone)]
pub struct UrlReplace {
    pub old_prefix: String,
    pub new_prefix: String,
}

pub fn find_config_file(search_path: PathBuf) -> Result<PathBuf> {
    let mut current = search_path;
    loop {
        let deps_file = current.join("DEPS");
        if deps_file.exists() {
            return Ok(deps_file);
        }
        
        if let Some(parent) = current.parent() {
            if parent == current {
                break;
            }
            current = parent.to_path_buf();
        } else {
            break;
        }
    }
    
    anyhow::bail!("DEPS file not found")
}

pub fn parse(
    config_file: &Path,
    tool_version: &str,
    platform: &str,
    url_replace_list: Option<&Vec<UrlReplace>>,
) -> Result<ParsedConfig> {
    if !config_file.exists() {
        anyhow::bail!("Config file does not exist: {}", config_file.display());
    }
    
    let content = utils::read_file(config_file)?;
    let deps_config: DepsConfig = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse DEPS file: {}", config_file.display()))?;
    
    let project_path = config_file.parent().unwrap();
    let config_version = deps_config.version.as_deref().unwrap_or("0.0.0");
    
    // 检查版本要求
    if utils::compare_version(tool_version, config_version) < 0 {
        anyhow::bail!(
            "DEPS config requires a higher version of depctl tool.\n\
            Requires version: {}\n\
            Current version: {}",
            config_version,
            tool_version
        );
    }
    
    let raw_vars = deps_config.vars.as_ref().cloned().unwrap_or_default();
    // 解析 vars
    let vars = resolve_vars(raw_vars)?;
    
    // 过滤平台特定的配置
    let repos = filter_by_platform(&deps_config.repos, platform);
    let files = filter_by_platform(&deps_config.files, platform);
    let actions = filter_by_platform(&deps_config.actions, platform);
    let linkfiles = filter_by_platform(&deps_config.linkfiles, platform);
    let copyfiles = filter_by_platform(&deps_config.copyfiles, platform);
    
    // 解析 repos
    let parsed_repos = parse_repos(&repos, &vars, project_path, url_replace_list)?;
    
    // 解析 files
    let parsed_files = parse_files(&files, &vars, project_path, url_replace_list)?;
    
    // 解析 actions
    let parsed_actions = parse_actions(&actions, &vars, project_path)?;
    
    // 解析 linkfiles
    let parsed_linkfiles = parse_linkfiles(&linkfiles, &vars, project_path)?;
    
    // 解析 copyfiles
    let parsed_copyfiles = parse_copyfiles(&copyfiles, &vars, project_path)?;
    
    Ok(ParsedConfig {
        version: config_version.to_string(),
        repos: parsed_repos,
        files: parsed_files,
        actions: parsed_actions,
        linkfiles: parsed_linkfiles,
        copyfiles: parsed_copyfiles,
    })
}

fn filter_by_platform<'a, T>(items: &'a Option<HashMap<String, Vec<T>>>, platform: &str) -> Vec<&'a T> {
    let mut result = Vec::new();
    if let Some(ref items_map) = items {
        for (key, values) in items_map {
            if key == "common" || key == platform {
                result.extend(values.iter());
            }
        }
    }
    result
}

fn parse_repos(
    repos: &[&RepoItem],
    vars: &HashMap<String, String>,
    project_path: &Path,
    url_replace_list: Option<&Vec<UrlReplace>>,
) -> Result<Vec<ParsedRepoItem>> {
    let mut result = Vec::new();
    for item in repos {
        let mut url = format_string(&item.url, vars);
        url = apply_url_replace(&url, url_replace_list);
        let commit = format_string(&item.commit, vars);
        let dir_str = format_string(&item.dir, vars);
        let dir = project_path.join(dir_str);
        
        result.push(ParsedRepoItem {
            url,
            commit,
            dir: dir.canonicalize().unwrap_or(dir),
        });
    }
    Ok(result)
}

fn parse_files(
    files: &[&FileItem],
    vars: &HashMap<String, String>,
    project_path: &Path,
    url_replace_list: Option<&Vec<UrlReplace>>,
) -> Result<Vec<ParsedFileItem>> {
    let mut result = Vec::new();
    for item in files {
        let mut url = format_string(&item.url, vars);
        url = apply_url_replace(&url, url_replace_list);
        let dir_str = format_string(&item.dir, vars);
        let dir = project_path.join(dir_str);
        let dir_canonicalized = dir.canonicalize().unwrap_or(dir.clone());
        let hash = utils::get_hash(&url);
        
        let url_without_query: String = url.split('?').next().unwrap_or(&url).to_string();
        let path_buf = PathBuf::from(&url_without_query);
        let file_name = path_buf
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        // hash_file 应该基于 canonicalized 的 dir，确保路径一致
        let hash_file = dir_canonicalized.join(format!(".{}.sha1", file_name));
        
        let unzip = item.unzip;
        
        let multipart = item.multipart.as_ref().map(|parts| {
            parts.iter().map(|p| format_string(p, vars)).collect()
        });
        
        result.push(ParsedFileItem {
            url,
            dir: dir_canonicalized,
            hash,
            hash_file,
            unzip,
            multipart,
            timeout: item.timeout.unwrap_or(15000),
        });
    }
    Ok(result)
}

fn parse_actions(
    actions: &[&ActionItem],
    vars: &HashMap<String, String>,
    project_path: &Path,
) -> Result<Vec<ParsedActionItem>> {
    let mut result = Vec::new();
    for item in actions {
        let command = format_string(&item.command, vars);
        let dir_str = format_string(&item.dir, vars);
        let dir = project_path.join(dir_str);
        
        result.push(ParsedActionItem {
            command,
            dir: dir.canonicalize().unwrap_or(dir),
        });
    }
    Ok(result)
}

fn parse_linkfiles(
    linkfiles: &[&LinkFileItem],
    vars: &HashMap<String, String>,
    project_path: &Path,
) -> Result<Vec<ParsedLinkFileItem>> {
    let mut result = Vec::new();
    for item in linkfiles {
        let src_str = format_string(&item.src, vars);
        let dest_str = format_string(&item.dest, vars);
        let src = project_path.join(src_str);
        let dest = project_path.join(dest_str);
        
        // 对于 linkfiles，源路径不应该 canonicalize，保持原始路径
        // 目标路径需要确保父目录存在，但目标本身是软链接，不需要 canonicalize
        result.push(ParsedLinkFileItem {
            src,  // 保持原始路径，不解析软链接
            dest, // 保持原始路径
        });
    }
    Ok(result)
}

fn parse_copyfiles(
    copyfiles: &[&CopyFileItem],
    vars: &HashMap<String, String>,
    project_path: &Path,
) -> Result<Vec<ParsedCopyFileItem>> {
    let mut result = Vec::new();
    for item in copyfiles {
        let src_str = format_string(&item.src, vars);
        let dest_str = format_string(&item.dest, vars);
        let src = project_path.join(src_str);
        let dest = project_path.join(dest_str);
        
        result.push(ParsedCopyFileItem {
            src: src.canonicalize().unwrap_or(src),
            dest: dest.canonicalize().unwrap_or(dest),
        });
    }
    Ok(result)
}

/// 解析 vars 变量：支持变量间引用（A 依赖 B）和系统环境变量
/// - 变量值中的 ${VAR} 会先查找 vars 中的 VAR，再查找环境变量
/// - 迭代求值直到稳定，检测循环引用
pub(crate) fn resolve_vars(raw: HashMap<String, String>) -> Result<HashMap<String, String>> {
    let mut resolved = HashMap::new();
    const MAX_ITERATIONS: usize = 100;

    for _ in 0..MAX_ITERATIONS {
        let mut changed = false;
        for (k, v) in &raw {
            let value: &str = resolved.get(k).map(String::as_str).unwrap_or(v.as_str());
            let new_value = substitute_vars_with_env(value, &resolved);
            if new_value != value || !resolved.contains_key(k) {
                resolved.insert(k.clone(), new_value);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    // 检查循环引用：若某 var 的值仍包含对同批次 vars 的引用，则可能为循环
    for (k, v) in &resolved {
        if !v.contains("${") {
            continue;
        }
        // 提取 ${X} 中的 X，若 X 在 vars 中且其值也含 ${，则可能是循环
        let mut i = 0;
        let chars: Vec<char> = v.chars().collect();
        while i < chars.len().saturating_sub(2) {
            if chars[i] == '$' && chars[i + 1] == '{' {
                i += 2;
                let start = i;
                while i < chars.len() && chars[i] != '}' {
                    i += 1;
                }
                if i < chars.len() {
                    let ref_key: String = chars[start..i].iter().collect();
                    if raw.contains_key(&ref_key) {
                        anyhow::bail!(
                            "Circular var reference: '{}' -> '{}'",
                            k,
                            ref_key
                        );
                    }
                }
            }
            i += 1;
        }
    }

    Ok(resolved)
}

/// 仅用于 resolve_vars 阶段：替换 ${VAR}，支持 vars + 环境变量
fn substitute_vars_with_env(text: &str, vars: &HashMap<String, String>) -> String {
    let mut result = String::new();
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();
    while i < chars.len() {
        if i < chars.len() - 1 && chars[i] == '$' && chars[i + 1] == '{' {
            i += 2;
            let start = i;
            while i < chars.len() && chars[i] != '}' {
                i += 1;
            }
            if i < chars.len() {
                let key: String = chars[start..i].iter().collect();
                let value: String = if let Some(v) = vars.get(&key) {
                    v.clone()
                } else if let Ok(ev) = std::env::var(&key) {
                    ev
                } else {
                    format!("${{{}}}", key)
                };
                result.push_str(&value);
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

/// 仅从 vars 查找并替换 ${VAR}（用于 repos、files 等，vars 已完整求值）
pub(crate) fn substitute_vars(text: &str, vars: &HashMap<String, String>) -> String {
    let mut result = String::new();
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();
    while i < chars.len() {
        if i < chars.len() - 1 && chars[i] == '$' && chars[i + 1] == '{' {
            i += 2;
            let start = i;
            while i < chars.len() && chars[i] != '}' {
                i += 1;
            }
            if i < chars.len() {
                let key: String = chars[start..i].iter().collect();
                let value: String = vars
                    .get(&key)
                    .cloned()
                    .unwrap_or_else(|| format!("${{{}}}", key));
                result.push_str(&value);
                i += 1;
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn format_string(text: &str, vars: &HashMap<String, String>) -> String {
    substitute_vars(text, vars)
}

fn apply_url_replace(url: &str, url_replace_list: Option<&Vec<UrlReplace>>) -> String {
    if let Some(replace_list) = url_replace_list {
        for replace in replace_list {
            if url.starts_with(&replace.old_prefix) {
                return format!("{}{}", replace.new_prefix, &url[replace.old_prefix.len()..]);
            }
        }
    }
    url.to_string()
}

pub fn parse_mirror(mirror_str: &str) -> Result<Vec<UrlReplace>> {
    let mut result = Vec::new();
    let pairs: Vec<&str> = mirror_str.split(',').collect();
    
    for pair in pairs {
        let parts: Vec<&str> = pair.split("->").collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            anyhow::bail!(
                "Invalid mirror format. Expected: 'old1->new1,old2->new2'\n\
                Each pair should be: 'old_prefix->new_prefix'"
            );
        }
        result.push(UrlReplace {
            old_prefix: parts[0].to_string(),
            new_prefix: parts[1].to_string(),
        });
    }
    
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_resolve_vars_var_to_var() {
        let mut raw = HashMap::new();
        raw.insert("GIT_DOMAIN".to_string(), "github.com".to_string());
        raw.insert(
            "GITHUB_DOMAIN".to_string(),
            "https://${GIT_DOMAIN}".to_string(),
        );
        let resolved = resolve_vars(raw).unwrap();
        assert_eq!(resolved.get("GIT_DOMAIN").unwrap(), "github.com");
        assert_eq!(
            resolved.get("GITHUB_DOMAIN").unwrap(),
            "https://github.com"
        );
    }

    #[test]
    fn test_resolve_vars_chained() {
        let mut raw = HashMap::new();
        raw.insert("A".to_string(), "1".to_string());
        raw.insert("B".to_string(), "${A}".to_string());
        raw.insert("C".to_string(), "prefix-${B}-suffix".to_string());
        let resolved = resolve_vars(raw).unwrap();
        assert_eq!(resolved.get("A").unwrap(), "1");
        assert_eq!(resolved.get("B").unwrap(), "1");
        assert_eq!(resolved.get("C").unwrap(), "prefix-1-suffix");
    }

    #[test]
    fn test_resolve_vars_circular() {
        let mut raw = HashMap::new();
        raw.insert("A".to_string(), "${B}".to_string());
        raw.insert("B".to_string(), "${A}".to_string());
        let err = resolve_vars(raw).unwrap_err();
        assert!(err.to_string().contains("Circular"));
    }

    #[test]
    fn test_substitute_vars_from_map() {
        let mut vars = HashMap::new();
        vars.insert("X".to_string(), "foo".to_string());
        assert_eq!(substitute_vars("${X}-bar", &vars), "foo-bar");
    }

    #[test]
    fn test_substitute_vars_unresolved_kept() {
        let vars = HashMap::new();
        assert_eq!(
            substitute_vars("${NOT_EXIST}", &vars),
            "${NOT_EXIST}"
        );
    }

    #[test]
    fn test_substitute_vars_no_env_fallback() {
        // 后续流程仅从 vars 取值，不查环境变量
        std::env::set_var("DEPCTL_TEST_ONLY_ENV", "from_env");
        let vars = HashMap::new();
        assert_eq!(
            substitute_vars("${DEPCTL_TEST_ONLY_ENV}", &vars),
            "${DEPCTL_TEST_ONLY_ENV}"
        );
        std::env::remove_var("DEPCTL_TEST_ONLY_ENV");
    }

    #[test]
    fn test_resolve_vars_env_var() {
        std::env::set_var("DEPCTL_TEST_ENV_VAR", "env_value");
        let mut raw = HashMap::new();
        raw.insert("PREFIX".to_string(), "${DEPCTL_TEST_ENV_VAR}".to_string());
        let resolved = resolve_vars(raw).unwrap();
        assert_eq!(resolved.get("PREFIX").unwrap(), "env_value");
        std::env::remove_var("DEPCTL_TEST_ENV_VAR");
    }

    #[test]
    fn test_parse_deps_with_var_deps() {
        std::env::set_var("DEPCTL_TEST_ENV_GIT_SCHEME_VAR", "https");
        let dir = std::env::temp_dir().join("depctl_test_vars");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let deps_file = dir.join("DEPS");
        let content = r#"{
            "version": "1.0.0",
            "vars": {
                "GIT_DOMAIN": "github.com",
                "GITHUB_DOMAIN": "${DEPCTL_TEST_ENV_GIT_SCHEME_VAR}://${GIT_DOMAIN}"
            },
            "repos": {"common": [{"url": "${GITHUB_DOMAIN}/a/b.git", "commit": "abc", "dir": "vendor"}]}
        }"#;
        fs::File::create(&deps_file)
            .unwrap()
            .write_all(content.as_bytes())
            .unwrap();

        let parsed = parse(
            &deps_file,
            "2.0.0",
            "mac",
            None,
        )
        .unwrap();

        assert_eq!(parsed.repos.len(), 1);
        assert_eq!(parsed.repos[0].url, "https://github.com/a/b.git");

        let _ = fs::remove_dir_all(&dir);
        std::env::remove_var("DEPCTL_TEST_ENV_GIT_SCHEME_VAR");
    }
}
