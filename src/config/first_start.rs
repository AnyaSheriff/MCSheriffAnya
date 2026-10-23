//! Выбор версии до создания мира, сетевых слушателей и загрузки модулей.
use std::{fs,io::{self,BufRead,Write},path::Path};

const PENDING: &str = "choose";

/// Существующие установки продолжают работать с прежними настройками.
/// Новая установка без терминала получает незаполненный конфиг и останавливается.
pub fn prepare(root: &Path, interactive: bool, input: &mut impl BufRead, output: &mut impl Write) -> io::Result<bool> {
    let path=root.join(super::mcsheriffanya::FILE);
    // Панели заранее создают пустую world и неполный mcsa.properties.
    // Такая папка ещё не означает, что сервер уже проходил первый запуск.
    let old_world=match fs::read_dir(root.join("world")) {
        Ok(mut entries)=>entries.next().transpose()?.is_some(),
        Err(error) if error.kind()==io::ErrorKind::NotFound=>false,
        Err(error)=>return Err(error),
    };
    let existing=old_world || super::mcsheriffanya::OLD_FILES.iter().any(|name|root.join(name).exists());
    let mut text=match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind()==io::ErrorKind::NotFound => {
            if existing { return Ok(true); }
            fs::create_dir_all(path.parent().expect("папка настроек"))?;
            let text=format!("# Перед первым запуском выберите основную версию Minecraft Java.\n# Доступные в этой сборке: {}, 26.3 (новый мир).\n# После выбора запустите ядро снова.\njava-version={PENDING}\nsend-error-reports={PENDING}\n",crate::network::VERSION_NAME);
            let mut file=fs::OpenOptions::new().write(true).create_new(true).open(&path)?;
            file.write_all(text.as_bytes())?; file.sync_all()?; text
        }
        Err(error) => return Err(error),
    };
    if !existing {
        let values=super::server_properties::parse_properties(&text);
        let mut changed=false;
        for key in ["java-version","send-error-reports"] {
            if !values.contains_key(key) {
                if !text.ends_with('\n'){text.push('\n');}
                text.push_str(&format!("{key}={PENDING}\n"));changed=true;
            }
        }
        if changed {fs::write(&path,&text)?;}
    }
    let values=super::server_properties::parse_properties(&text);
    let choose_version=values.get("java-version").is_some_and(|v| v == PENDING || (!existing && !matches!(v.as_str(), "26.1.2" | "26.3")));
    let choose_reports=values.get("send-error-reports").is_some_and(|v| v == PENDING || (!existing && !matches!(v.as_str(),"true"|"false")));
    if !choose_version && !choose_reports { return Ok(true); }
    writeln!(output,"Первоначальная настройка MCSheriffAnya. Мир ещё не создан.")?;
    if choose_version {
        writeln!(output,"Выберите основную версию Minecraft Java: 1. {}  2. 26.3 (новый мир)",crate::network::VERSION_NAME)?;
    }
    if choose_reports {
        writeln!(output,"Отправлять разработчикам анонимные отчёты об ошибках?")?;
        writeln!(output,"Отчёт включает ошибку и до 50 последних строк лога; личные данные очищаются. Отправка выключена до вашего согласия.")?;
    }
    if !interactive {
        writeln!(output,"В панели откройте config/mcsa.properties:")?;
        if choose_version { writeln!(output,"  java-version={} или 26.3 — основная версия игры",crate::network::VERSION_NAME)?; }
        if choose_reports { writeln!(output,"  send-error-reports=true или false — отправлять или не отправлять отчёты")?; }
        writeln!(output,"Укажите выбранные значения и запустите сервер снова. Запуск остановлен до выбора.")?;
        return Ok(false);
    }
    let mut answers=Vec::new();
    if choose_version {
        loop {
            write!(output,"Номер или версия (отмена — stop): ")?;output.flush()?;
            let mut line=String::new();if input.read_line(&mut line)?==0 { return Ok(false); }
            let choice=line.trim();
            if matches!(choice,"stop"|"cancel"|"отмена") { return Ok(false); }
            if matches!(choice, "2" | "26.3") {
                answers.push(("java-version", "26.3")); break;
            }
            if choice == "1" || choice == crate::network::VERSION_NAME {
                answers.push(("java-version",crate::network::VERSION_NAME));break;
            }
            writeln!(output,"Укажите версию из списка. Пустой ввод не выбирает версию автоматически.")?;
        }
    }
    if choose_reports {
        loop {
            write!(output,"Отправлять отчёты? да / нет (отмена — stop): ")?;output.flush()?;
            let mut line=String::new();if input.read_line(&mut line)?==0 { return Ok(false); }
            match line.trim().to_lowercase().as_str() {
                "да"|"yes"|"true" => { answers.push(("send-error-reports","true"));break; }
                "нет"|"no"|"false" => { answers.push(("send-error-reports","false"));break; }
                "stop"|"cancel"|"отмена" => return Ok(false),
                _ => writeln!(output,"Ответьте да или нет. Пустой ввод не считается согласием.")?,
            }
        }
    }
    // Записываем оба ответа только после завершения выбора; остальные строки сохраняем.
    let latest=fs::read_to_string(&path)?;
    let result=latest.lines().map(|line| {
        if let Some((key,value))=answers.iter().find(|(key,_)|line.trim().split_once('=').is_some_and(|(name,_)|name.trim()==*key)) {
            format!("{key}={value}")
        } else { line.to_owned() }
    }).collect::<Vec<_>>().join("\n")+"\n";
    fs::write(&path,result)?;
    writeln!(output,"Выбор сохранён в config/mcsa.properties.")?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn root(name:&str)->std::path::PathBuf {
        let path=std::path::PathBuf::from(format!("tmp/first-start-{name}-{}",std::process::id()));
        if path.exists(){fs::remove_dir_all(&path).unwrap();} fs::create_dir_all(&path).unwrap();path
    }
    #[test]
    fn headless_requires_an_explicit_selection_without_creating_a_world() {
        let root=root("headless");let mut output=Vec::new();
        for _ in 0..2 { assert!(!prepare(&root,false,&mut &b""[..],&mut output).unwrap());assert!(!root.join("world").exists()); }
        fs::write(root.join(super::super::mcsheriffanya::FILE),format!("java-version={}\nsend-error-reports=false\n",crate::network::VERSION_NAME)).unwrap();
        assert!(prepare(&root,false,&mut &b""[..],&mut output).unwrap());
        assert!(!root.join("world").exists());fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn interactive_selection_is_saved_and_not_repeated() {
        let root=root("interactive");let mut output=Vec::new();
        assert!(prepare(&root,true,&mut &b"\n26.4\n1\n\nno\n"[..],&mut output).unwrap());
        assert!(fs::read_to_string(root.join(super::super::mcsheriffanya::FILE)).unwrap().contains(&format!("java-version={}",crate::network::VERSION_NAME)));
        assert!(prepare(&root,true,&mut &b""[..],&mut Vec::new()).unwrap());fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reports_require_explicit_consent_and_partial_answers_do_not_start() {
        let root=root("consent");let mut output=Vec::new();
        assert!(!prepare(&root,true,&mut &b"1\n\n"[..],&mut output).unwrap());
        let path=root.join(super::super::mcsheriffanya::FILE);
        assert!(fs::read_to_string(&path).unwrap().contains("send-error-reports=choose"));
        assert!(!root.join("world").exists());
        assert!(prepare(&root,true,&mut &b"1\nyes\n"[..],&mut output).unwrap());
        assert!(fs::read_to_string(path).unwrap().contains("send-error-reports=true"));fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cancelling_and_existing_installations_are_preserved() {
        let root=root("cancel");
        assert!(!prepare(&root,true,&mut &b"stop\n"[..],&mut Vec::new()).unwrap());
        assert!(!root.join("world").exists());
        fs::create_dir(root.join("world")).unwrap();fs::write(root.join("world/level.dat"),b"existing-world-marker").unwrap();
        let path=root.join(super::super::mcsheriffanya::FILE);let previous="auth-enabled=false\n";fs::write(&path,previous).unwrap();
        assert!(prepare(&root,false,&mut &b""[..],&mut Vec::new()).unwrap());assert_eq!(fs::read_to_string(path).unwrap(),previous);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn panel_created_empty_world_and_partial_config_still_require_selection() {
        let root=root("panel");fs::create_dir(root.join("world")).unwrap();fs::create_dir(root.join("config")).unwrap();
        let path=root.join(super::super::mcsheriffanya::FILE);fs::write(&path,"bedrock-port=0\nauth-enabled=false\n").unwrap();
        assert!(!prepare(&root,false,&mut &b""[..],&mut Vec::new()).unwrap());
        let text=fs::read_to_string(&path).unwrap();assert!(text.contains("java-version=choose"));assert!(text.contains("send-error-reports=choose"));assert!(text.contains("bedrock-port=0"));
        assert!(prepare(&root,true,&mut &b"1\nno\n"[..],&mut Vec::new()).unwrap());
        assert!(fs::read_to_string(&path).unwrap().contains("send-error-reports=false"));fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn malformed_fresh_consent_is_not_a_completed_choice() {
        let root=root("invalid-consent");fs::create_dir(root.join("config")).unwrap();
        let path=root.join(super::super::mcsheriffanya::FILE);
        for invalid in ["", "maybe", "yes", "TRUE"] {
            let text=format!("java-version={}\nsend-error-reports={invalid}\nauth-enabled=false\n",crate::network::VERSION_NAME);
            fs::write(&path,&text).unwrap();let mut output=Vec::new();
            assert!(!prepare(&root,false,&mut &b""[..],&mut output).unwrap());
            assert_eq!(fs::read_to_string(&path).unwrap(),text);
            assert!(!root.join("world").exists());
            assert!(String::from_utf8(output).unwrap().contains("send-error-reports=true или false"));
            assert!(prepare(&root,true,&mut &b"no\n"[..],&mut Vec::new()).unwrap());
            let saved=fs::read_to_string(&path).unwrap();
            assert!(saved.contains("send-error-reports=false"));assert!(saved.contains("auth-enabled=false"));
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unavailable_fresh_version_requires_selection_before_start() {
        let root=root("invalid-version");fs::create_dir(root.join("config")).unwrap();
        let path=root.join(super::super::mcsheriffanya::FILE);
        for invalid in ["", "invalid", "26.4"] {
            fs::write(&path,format!("java-version={invalid}\nsend-error-reports=false\n")).unwrap();
            assert!(!prepare(&root,false,&mut &b""[..],&mut Vec::new()).unwrap());
            assert!(!root.join("world").exists());
            assert!(prepare(&root,true,&mut &b"1\n"[..],&mut Vec::new()).unwrap());
            assert!(fs::read_to_string(&path).unwrap().contains(&format!("java-version={}",crate::network::VERSION_NAME)));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_263_choice_survives_headless_restart() {
        let root = root("263-choice");
        assert!(prepare(&root, true, &mut &b"2\nno\n"[..], &mut Vec::new()).unwrap());
        assert!(fs::read_to_string(root.join(super::super::mcsheriffanya::FILE)).unwrap().contains("java-version=26.3"));
        assert!(prepare(&root, false, &mut &b""[..], &mut Vec::new()).unwrap());
        assert!(!root.join("world").exists());
        fs::remove_dir_all(root).unwrap();
    }

}
