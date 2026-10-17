use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Client { Desktop, Code, Codex, Cursor }
impl Client {
    pub fn name(self) -> &'static str {
        match self { Self::Desktop => "Claude Desktop", Self::Code => "Claude Code", Self::Codex => "Codex CLI", Self::Cursor => "Cursor" }
    }
}

pub struct Locations { pub appdata: PathBuf, pub home: PathBuf }
impl Locations {
    pub fn system() -> Result<Self, String> {
        Ok(Self {
            appdata: std::env::var_os("APPDATA").map(PathBuf::from).ok_or("APPDATA introuvable")?,
            home: std::env::var_os("USERPROFILE").map(PathBuf::from).ok_or("USERPROFILE introuvable")?,
        })
    }
    pub fn path(&self, client: Client) -> PathBuf {
        match client {
            Client::Desktop => self.appdata.join("Claude/claude_desktop_config.json"),
            Client::Code => self.home.join(".claude.json"),
            Client::Codex => self.home.join(".codex/config.toml"),
            Client::Cursor => self.home.join(".cursor/mcp.json"),
        }
    }
}

pub fn connector(app_config: &Path) -> Result<PathBuf, String> {
    let exe = app_config.with_file_name("deckforge-mcp.exe");
    if !exe.is_file() { return Err(format!("Connecteur introuvable : {}", exe.display())); }
    Ok(exe)
}

pub fn configured(client: Client, locations: &Locations) -> bool {
    let Ok(text) = std::fs::read_to_string(locations.path(client)) else { return false };
    if client == Client::Codex { return codex_block(&text).is_some(); }
    serde_json::from_str::<Value>(&text).ok().is_some_and(|v| v.get("mcpServers").and_then(|v| v.get("deckforge")).is_some())
}

fn atomic_save(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().ok_or("Chemin de configuration invalide")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let tmp = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    if path.exists() {
        std::fs::copy(path, path.with_file_name(format!("{}.bak-deckforge", path.file_name().unwrap().to_string_lossy()))).map_err(|e| e.to_string())?;
    }
    std::fs::write(tmp.path(), content).map_err(|e| e.to_string())?;
    tmp.persist(path).map_err(|e| e.error.to_string())?;
    Ok(())
}

fn codex_block(text: &str) -> Option<(usize, usize)> {
    let mut offset=0;
    let mut start=None;
    for line in text.split_inclusive('\n') {
        let trimmed=line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if let Some(begin)=start { return Some((begin, offset)); }
            if trimmed=="[mcp_servers.deckforge]" { start=Some(offset); }
        }
        offset+=line.len();
    }
    start.map(|begin|(begin,text.len()))
}

pub fn configure(client: Client, locations: &Locations, exe: &Path, remove: bool) -> Result<String, String> {
    if !remove && !exe.is_file() { return Err(format!("Connecteur introuvable : {}", exe.display())); }
    let path=locations.path(client);
    let old=match std::fs::read_to_string(&path) { Ok(s)=>s, Err(e) if e.kind()==std::io::ErrorKind::NotFound => String::new(), Err(e)=>return Err(e.to_string()) };
    let next=if client==Client::Codex {
        let mut text=old.clone();
        if let Some((start,end))=codex_block(&text) { text.replace_range(start..end, ""); }
        if !remove {
            if !text.is_empty() && !text.ends_with('\n') { text.push('\n'); }
            text.push_str("[mcp_servers.deckforge]\ncommand = ");
            text.push_str(&format!("{}\n", toml_literal(&exe.to_string_lossy())));
        }
        text
    } else {
        let mut root:Value=if old.trim().is_empty() { json!({}) } else { serde_json::from_str(&old).map_err(|e|format!("JSON invalide, fichier préservé : {e}"))? };
        let object=root.as_object_mut().ok_or("Configuration JSON non objet, fichier préservé")?;
        if !object.contains_key("mcpServers") && !remove { object.insert("mcpServers".into(),json!({})); }
        if let Some(servers)=object.get_mut("mcpServers") {
            let map=servers.as_object_mut().ok_or("mcpServers invalide, fichier préservé")?;
            if remove { map.remove("deckforge"); }
            else {
                let mut value=json!({"command":exe.to_string_lossy(),"args":[]});
                if client==Client::Code { value["type"]=json!("stdio"); }
                map.insert("deckforge".into(),value);
            }
        }
        format!("{}\n",serde_json::to_string_pretty(&root).map_err(|e|e.to_string())?)
    };
    if old==next { return Ok(format!("{} : aucun changement",client.name())); }
    if remove && old.is_empty() { return Ok(format!("{} : rien à retirer",client.name())); }
    atomic_save(&path,&next)?;
    Ok(format!("{} : {}. {} Redémarre le client.",client.name(),if remove {"retiré"} else {"configuré"},if old.is_empty() {"Nouveau fichier.".to_string()} else {format!("Sauvegarde : {}.",path.with_file_name(format!("{}.bak-deckforge",path.file_name().unwrap().to_string_lossy())).display())}))
}
fn toml_literal(path: &str) -> String {
    if !path.contains('\'') && !path.contains('\n') && !path.contains('\r') { format!("'{path}'") }
    else { serde_json::to_string(path).unwrap_or_default() }
}

pub fn find_claude(locations: &Locations, path_env: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let candidates=[locations.home.join(".local/bin/claude.exe"),locations.appdata.join("npm/claude.cmd")];
    for path in candidates { if path.is_file() {return Some(path);} }
    let paths=path_env.map(std::env::split_paths)?;
    for dir in paths {for name in ["claude.exe","claude.cmd"] {let path=dir.join(name);if path.is_file(){return Some(path);}}}
    None
}
fn claude_cli(cli: &Path, args: &[&str]) -> Result<String,String> {
    use std::process::{Command,Stdio};
    use std::time::{Duration,Instant};
    let mut command=if cli.extension().is_some_and(|v|v.eq_ignore_ascii_case("cmd")) {
        let mut c=Command::new("cmd.exe");c.arg("/C").arg(cli);c
    } else {Command::new(cli)};
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)] {use std::os::windows::process::CommandExt;command.creation_flags(0x08000000);}
    let mut child=command.spawn().map_err(|e|e.to_string())?;
    let start=Instant::now();
    while child.try_wait().map_err(|e|e.to_string())?.is_none() {
        if start.elapsed()>Duration::from_secs(20) {let _=child.kill();let _=child.wait();return Err("Claude Code : délai de 20 s dépassé".into());}
        std::thread::sleep(Duration::from_millis(50));
    }
    let output=child.wait_with_output().map_err(|e|e.to_string())?;
    let message=format!("{} {}",String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr));
    if output.status.success(){Ok(message)}else{Err(format!("Claude Code : {}",message.trim()))}
}
pub fn configure_code(locations: &Locations, exe: &Path, remove: bool) -> Result<String,String> {
    if let Some(cli)=find_claude(locations,std::env::var_os("PATH").as_deref()) {
        let config=locations.path(Client::Code);
        if config.exists(){std::fs::copy(&config,config.with_file_name(".claude.json.bak-deckforge")).map_err(|e|e.to_string())?;}
        if remove {return claude_cli(&cli,&["mcp","remove","--scope","user","deckforge"]);}
        if !exe.is_file(){return Err("Connecteur introuvable".into());}
        if configured(Client::Code,locations) {let _=claude_cli(&cli,&["mcp","remove","--scope","user","deckforge"])?;}
        let path=exe.to_str().ok_or("Chemin du connecteur invalide")?;
        let output=claude_cli(&cli,&["mcp","add","--scope","user","deckforge","--",path])?;
        Ok(format!("Claude Code configuré : {output}. Redémarre Claude Code."))
    } else {configure(Client::Code,locations,exe,remove)}
}
pub fn test_connection(app_config: &Path) -> Result<String, String> {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::time::Duration;
    let exe=connector(app_config)?;
    let mut command=Command::new(exe);
    command.current_dir(std::env::temp_dir()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)] { use std::os::windows::process::CommandExt; command.creation_flags(0x08000000); }
    let mut child=command.spawn().map_err(|e|format!("Connecteur impossible à lancer : {e}"))?;
    let stdout=child.stdout.take().ok_or("Réponse stdio indisponible")?;
    let (tx,rx)=mpsc::channel();
    let reader=std::thread::spawn(move || {
        let mut lines=BufReader::new(stdout).lines();
        for _ in 0..2 { let _=tx.send(lines.next().transpose().map_err(|e|e.to_string())); }
    });
    let result=(||{
        let input=child.stdin.as_mut().ok_or("Entrée stdio indisponible")?;
        writeln!(input,"{}",serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"DeckForge","version":"0.5"}}})).map_err(|e|e.to_string())?;
        writeln!(input,"{}",serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})).map_err(|e|e.to_string())?;
        input.flush().map_err(|e|e.to_string())?;
        let init=rx.recv_timeout(Duration::from_secs(3)).map_err(|_|"Délai dépassé : initialize")??.ok_or("Pas de réponse à initialize")?;
        let v:Value=serde_json::from_str(&init).map_err(|_|"Réponse initialize invalide")?;
        if v["result"]["protocolVersion"].is_null() { return Err("Connecteur ancien ou incompatible (initialize)".into()); }
        let tools=rx.recv_timeout(Duration::from_secs(3)).map_err(|_|"Délai dépassé : tools/list")??.ok_or("Pas de réponse à tools/list")?;
        let v:Value=serde_json::from_str(&tools).map_err(|_|"Réponse tools/list invalide")?;
        let count=v["result"]["tools"].as_array().ok_or("Connecteur ancien : liste des outils absente")?.len();
        if count!=11 {return Err(format!("Version incompatible : {count} outils au lieu de 11"));}
        Ok(format!("✓ {count} outils disponibles"))
    })();
    let _=child.kill();let _=child.wait();let _=reader.join();
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn json_clients_preserve_existing_and_remove_only_own_entry() {
        let dir=tempfile::tempdir().unwrap();
        let loc=Locations{home:dir.path().join("home"),appdata:dir.path().join("roaming")};
        let exe=dir.path().join("deckforge-mcp.exe");std::fs::write(&exe,"").unwrap();
        for client in [Client::Desktop,Client::Code,Client::Cursor] {
            let path=loc.path(client);std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path,r#"{"other":42,"mcpServers":{"existing":{"command":"elsewhere"}}}"#).unwrap();
            configure(client,&loc,&exe,false).unwrap();
            let first=std::fs::read_to_string(&path).unwrap();
            configure(client,&loc,&exe,false).unwrap();
            assert_eq!(first,std::fs::read_to_string(&path).unwrap());
            assert!(first.contains("deckforge"));assert!(first.contains("existing"));
            configure(client,&loc,&exe,true).unwrap();
            let v:Value=serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            assert_eq!(v["other"],42);assert!(v["mcpServers"]["deckforge"].is_null());
            assert_eq!(v["mcpServers"]["existing"]["command"],"elsewhere");
            assert!(path.with_file_name(format!("{}.bak-deckforge",path.file_name().unwrap().to_string_lossy())).exists());
        }
    }
    #[test]
    fn cli_detection_uses_only_injected_paths() {
        let dir=tempfile::tempdir().unwrap();let loc=Locations{home:dir.path().join("home"),appdata:dir.path().join("app")};
        assert!(find_claude(&loc,Some(dir.path().as_os_str())).is_none());
        let cli=loc.home.join(".local/bin/claude.exe");std::fs::create_dir_all(cli.parent().unwrap()).unwrap();std::fs::write(&cli,"").unwrap();
        assert_eq!(find_claude(&loc,None),Some(cli));
    }
    #[test]
    fn connector_responds_with_eleven_tools_when_available() {
        let Ok(exe)=std::env::var("DECKFORGE_MCP_EXE") else {return};
        let dir=tempfile::tempdir().unwrap();
        let target=dir.path().join("deckforge-mcp.exe");std::fs::copy(exe,&target).unwrap();
        let config=dir.path().join("config.json");
        assert_eq!(test_connection(&config).unwrap(),"✓ 11 outils disponibles");
    }
    #[test]
    fn codex_block_is_replaced_without_touching_neighbors() {
        let dir=tempfile::tempdir().unwrap();let loc=Locations{home:dir.path().join("home"),appdata:dir.path().join("app")};
        let exe=dir.path().join("deckforge-mcp.exe");std::fs::write(&exe,"").unwrap();
        let path=loc.path(Client::Codex);std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path,"[other]\na = 1\n[mcp_servers.deckforge]\ncommand = 'old'\n[tail]\nb = 2\n").unwrap();
        configure(Client::Codex,&loc,&exe,false).unwrap();let s=std::fs::read_to_string(&path).unwrap();
        assert!(s.contains("[other]\na = 1"));assert!(s.contains("[tail]\nb = 2"));
        configure(Client::Codex,&loc,&exe,false).unwrap();assert_eq!(s,std::fs::read_to_string(&path).unwrap());
        configure(Client::Codex,&loc,&exe,true).unwrap();assert!(!std::fs::read_to_string(&path).unwrap().contains("deckforge]"));
    }
}
