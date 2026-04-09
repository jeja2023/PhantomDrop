use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    fn parse(value: &str) -> Result<Self, String> {
        let parts: Vec<&str> = value.trim().split('.').collect();
        if parts.len() != 3 {
            return Err(format!("无效的版本号格式: {}", value));
        }

        let major = parts[0]
            .parse::<u64>()
            .map_err(|_| format!("无效的主版本号: {}", value))?;
        let minor = parts[1]
            .parse::<u64>()
            .map_err(|_| format!("无效的次版本号: {}", value))?;
        let patch = parts[2]
            .parse::<u64>()
            .map_err(|_| format!("无效的修订版本号: {}", value))?;

        Ok(Self { major, minor, patch })
    }

    fn bump(&self, kind: &str) -> Result<Self, String> {
        match kind {
            "patch" => Ok(Self {
                major: self.major,
                minor: self.minor,
                patch: self.patch + 1,
            }),
            "minor" => Ok(Self {
                major: self.major,
                minor: self.minor + 1,
                patch: 0,
            }),
            "major" => Ok(Self {
                major: self.major + 1,
                minor: 0,
                patch: 0,
            }),
            _ => Err(format!("不支持的升级类型: {}", kind)),
        }
    }

    fn plain(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }

    fn tag(&self) -> String {
        format!("V{}", self.plain())
    }
}

struct ReleaseConfig {
    root: PathBuf,
    date: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("错误: {}", error);
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        print_usage();
        return Err("请提供 --set 或 --bump 参数".to_string());
    }

    let root = find_project_root()?;
    let config = ReleaseConfig {
        root,
        date: env::var("RELEASE_DATE").unwrap_or_else(|_| "2026-04-08".to_string()),
    };

    let current = read_current_version(&config.root)?;
    let target = parse_target_version(&args, &current)?;

    apply_release(&config, &current, &target)?;

    println!("✅ 版本已更新: {} -> {}", current.plain(), target.plain());
    Ok(())
}

fn print_usage() {
    eprintln!("用法:");
    eprintln!("  cargo run --manifest-path tools/release-tool/Cargo.toml -- --set 0.0.3");
    eprintln!("  cargo run --manifest-path tools/release-tool/Cargo.toml -- --bump patch");
    eprintln!("可选环境变量:");
    eprintln!("  RELEASE_DATE=2026-04-08");
}

fn find_project_root() -> Result<PathBuf, String> {
    let exe_path = env::current_exe().map_err(|error| format!("无法定位可执行文件: {error}"))?;
    let exe_dir = exe_path
        .parent()
        .ok_or("无法定位工具目录".to_string())?;

    exe_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or("无法推导项目根目录".to_string())
}

fn parse_target_version(args: &[String], current: &Version) -> Result<Version, String> {
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--set" => {
                let value = args.get(index + 1).ok_or("--set 需要跟一个版本号".to_string())?;
                return Version::parse(value);
            }
            "--bump" => {
                let value = args
                    .get(index + 1)
                    .ok_or("--bump 需要跟 patch/minor/major".to_string())?;
                return current.bump(value);
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            _ => {}
        }
        index += 1;
    }

    Err("未找到可用参数，请使用 --set 或 --bump".to_string())
}

fn read_current_version(root: &Path) -> Result<Version, String> {
    let path = root.join("core").join("Cargo.toml");
    let content = fs::read_to_string(&path).map_err(|error| format!("读取 {} 失败: {error}", path.display()))?;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("version = ") {
            return Version::parse(value.trim_matches('"'));
        }
    }

    Err("未在 core/Cargo.toml 中找到版本号".to_string())
}

fn apply_release(config: &ReleaseConfig, current: &Version, target: &Version) -> Result<(), String> {
    update_core_cargo_toml(config, current, target)?;
    update_core_cargo_lock(config, current, target)?;
    update_package_json(config.root.join("web").join("package.json"), current, target)?;
    update_package_lock(config.root.join("web").join("package-lock.json"), current, target)?;
    update_package_json(config.root.join("network").join("package.json"), current, target)?;
    update_sidebar_version(config, current, target)?;
    update_changelog(config, target)?;
    Ok(())
}

fn update_core_cargo_toml(config: &ReleaseConfig, current: &Version, target: &Version) -> Result<(), String> {
    let path = config.root.join("core").join("Cargo.toml");
    replace_in_file(
        &path,
        &format!("version = \"{}\"", current.plain()),
        &format!("version = \"{}\"", target.plain()),
    )
}

fn update_core_cargo_lock(config: &ReleaseConfig, current: &Version, target: &Version) -> Result<(), String> {
    let path = config.root.join("core").join("Cargo.lock");
    let content = fs::read_to_string(&path).map_err(|error| format!("读取 {} 失败: {error}", path.display()))?;
    let marker = format!("name = \"core\"\nversion = \"{}\"", current.plain());
    let replacement = format!("name = \"core\"\nversion = \"{}\"", target.plain());

    if !content.contains(&marker) {
        return Ok(());
    }

    let updated = content.replacen(&marker, &replacement, 1);
    fs::write(&path, updated).map_err(|error| format!("写入 {} 失败: {error}", path.display()))
}

fn update_package_json(path: PathBuf, current: &Version, target: &Version) -> Result<(), String> {
    replace_in_file(
        &path,
        &format!("\"version\": \"{}\"", current.plain()),
        &format!("\"version\": \"{}\"", target.plain()),
    )
}

fn update_package_lock(path: PathBuf, current: &Version, target: &Version) -> Result<(), String> {
    let content = fs::read_to_string(&path).map_err(|error| format!("读取 {} 失败: {error}", path.display()))?;
    let current_line = format!("\"version\": \"{}\",", current.plain());
    let target_line = format!("\"version\": \"{}\",", target.plain());
    let mut replacements = 0;
    let mut updated = String::with_capacity(content.len());

    for line in content.lines() {
        if replacements < 2 && line.trim() == current_line {
            updated.push_str(&line.replacen(&current.plain(), &target.plain(), 1));
            updated.push('\n');
            replacements += 1;
        } else {
            updated.push_str(line);
            updated.push('\n');
        }
    }

    if replacements == 0 {
        return Err(format!("未在 {} 中找到顶层版本号", path.display()));
    }

    if replacements == 1 {
        updated = updated.replacen(&current_line, &target_line, 1);
    }

    fs::write(&path, updated).map_err(|error| format!("写入 {} 失败: {error}", path.display()))
}

fn update_sidebar_version(config: &ReleaseConfig, current: &Version, target: &Version) -> Result<(), String> {
    let path = config.root.join("web").join("src").join("ui").join("Sidebar.tsx");
    replace_in_file(
        &path,
        &format!("CORE {}", current.tag()),
        &format!("CORE {}", target.tag()),
    )
}

fn update_changelog(config: &ReleaseConfig, target: &Version) -> Result<(), String> {
    let path = config.root.join("更新日志.md");
    let content = fs::read_to_string(&path).map_err(|error| format!("读取 {} 失败: {error}", path.display()))?;
    let target_heading = format!("## [{}] - {}", target.tag(), config.date);

    if content.contains(&target_heading) {
        return Ok(());
    }

    let separator = "\n---\n";
    let insert_at = content
        .find(separator)
        .map(|index| index + separator.len())
        .ok_or("未在更新日志中找到插入位置".to_string())?;

    let new_section = format!(
        "\n## [{}] - {}\n\n### Changed\n- 待补充\n",
        target.tag(),
        config.date
    );

    let mut updated = String::with_capacity(content.len() + new_section.len());
    updated.push_str(&content[..insert_at]);
    updated.push_str(&new_section);
    updated.push_str(&content[insert_at..]);

    if !updated.ends_with('\n') {
        updated.push('\n');
    }

    fs::write(&path, updated).map_err(|error| format!("写入 {} 失败: {error}", path.display()))
}

fn replace_in_file(path: &Path, from: &str, to: &str) -> Result<(), String> {
    let content = fs::read_to_string(path).map_err(|error| format!("读取 {} 失败: {error}", path.display()))?;
    if !content.contains(from) {
        return Err(format!("未在 {} 中找到目标内容: {}", path.display(), from));
    }

    let updated = content.replacen(from, to, 1);
    fs::write(path, updated).map_err(|error| format!("写入 {} 失败: {error}", path.display()))
}
