//! Signing and manifest helper for ZMark releases.
//!
//! The private key never enters the repository. `generate` writes it to the
//! path given on the command line; `sign` reads the same PKCS#8 file.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use ring::rand::SystemRandom;
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};

fn args() -> Vec<String> {
    std::env::args().skip(1).collect()
}

fn option(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn required(args: &[String], name: &str) -> Result<String, String> {
    option(args, name).ok_or_else(|| format!("missing {name}"))
}

fn load_key(path: &str) -> Result<Ed25519KeyPair, String> {
    let bytes = fs::read(path).map_err(|error| format!("read key: {error}"))?;
    Ed25519KeyPair::from_pkcs8(&bytes).map_err(|_| "invalid PKCS#8 Ed25519 key".to_string())
}

fn payload(args: &[String]) -> Result<String, String> {
    Ok(format!(
        "{}\n{}\n{}\n{}",
        required(args, "--version")?,
        required(args, "--url")?,
        required(args, "--sha256")?,
        required(args, "--published-at")?
    ))
}

fn run() -> Result<(), String> {
    let args = args();
    let command = args.first().ok_or("missing command")?.as_str();
    match command {
        "generate" => {
            let key_path = required(&args, "--key")?;
            let rng = SystemRandom::new();
            let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
                .map_err(|_| "key generation failed".to_string())?;
            if let Some(parent) = PathBuf::from(&key_path).parent() {
                fs::create_dir_all(parent).map_err(|error| format!("mkdir: {error}"))?;
            }
            fs::write(&key_path, pkcs8.as_ref()).map_err(|error| format!("write key: {error}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = fs::metadata(&key_path)
                    .map_err(|error| format!("stat key: {error}"))?
                    .permissions();
                permissions.set_mode(0o600);
                fs::set_permissions(&key_path, permissions)
                    .map_err(|error| format!("chmod key: {error}"))?;
            }
            let key = load_key(&key_path)?;
            println!("{}", hex::encode(key.public_key().as_ref()));
        }
        "pubkey" => {
            let key = load_key(&required(&args, "--key")?)?;
            println!("{}", hex::encode(key.public_key().as_ref()));
        }
        "payload" => {
            print!("{}", payload(&args)?);
        }
        "sign" => {
            let key = load_key(&required(&args, "--key")?)?;
            let payload_path = required(&args, "--payload-file")?;
            let payload = fs::read(&payload_path)
                .map_err(|error| format!("read payload: {error}"))?;
            println!("{}", hex::encode(key.sign(&payload).as_ref()));
        }
        "verify" => {
            let public_key = hex::decode(required(&args, "--pubkey")?)
                .map_err(|_| "invalid public key hex".to_string())?;
            let signature = hex::decode(required(&args, "--sig")?)
                .map_err(|_| "invalid signature hex".to_string())?;
            let payload_path = required(&args, "--payload-file")?;
            let payload = fs::read(&payload_path)
                .map_err(|error| format!("read payload: {error}"))?;
            UnparsedPublicKey::new(&ED25519, public_key)
                .verify(&payload, &signature)
                .map_err(|_| "signature verification failed".to_string())?;
            println!("ok");
        }
        "manifest" => {
            let notes = option(&args, "--notes").unwrap_or_default();
            let value = serde_json::json!({
                "version": required(&args, "--version")?,
                "url": required(&args, "--url")?,
                "sha256": required(&args, "--sha256")?,
                "published_at": required(&args, "--published-at")?,
                "notes": notes,
                "signature": required(&args, "--signature")?,
            });
            println!("{}", value);
        }
        other => return Err(format!("unknown command: {other}")),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("release_tool: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_matches_the_app_format() {
        let args = vec![
            "--version".into(),
            "0.2.1".into(),
            "--url".into(),
            "u".into(),
            "--sha256".into(),
            "h".into(),
            "--published-at".into(),
            "p".into(),
        ];
        assert_eq!(payload(&args).unwrap(), "0.2.1\nu\nh\np");
    }

    #[test]
    fn generated_key_signs_and_verifies() {
        let dir = std::env::temp_dir().join(format!(
            "zmark_release_tool_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("release.key");
        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
        fs::write(&key_path, pkcs8.as_ref()).unwrap();
        let key = load_key(key_path.to_str().unwrap()).unwrap();
        let public = hex::encode(key.public_key().as_ref());
        let message = b"0.2.1\nu\nh\np";
        let signature = hex::encode(key.sign(message).as_ref());
        UnparsedPublicKey::new(&ED25519, hex::decode(&public).unwrap())
            .verify(message, &hex::decode(&signature).unwrap())
            .unwrap();
        let _ = fs::remove_dir_all(&dir);
    }
}
