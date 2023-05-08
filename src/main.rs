use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug)]
struct Dependency {
    name: String,
    expected_hash: String,
}

impl Dependency {
    fn from(node: roxmltree::Node) -> Result<Self> {
        Ok(Dependency {
            name: node
                .attribute("Name")
                .context("missing attribute 'Name'")?
                .to_owned(),
            expected_hash: node
                .attribute("ExpectedHash")
                .context("missing attribute 'ExpectedHash'")?
                .to_owned(),
        })
    }
}

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let cache = Path::new("deps_cache");
    if !cache.exists() {
        bail!("./deps_cache directory does not exit");
    }
    if args.len() >= 3 {
        match args[1].as_str() {
            "cache" => {
                for path in &args[2..] {
                    println!("caching {}", path);
                    build_cache(cache, Path::new(path))?;
                }
                return Ok(());
            }
            "restore" => {
                for path in &args[2..] {
                    println!("restoring {}", path);
                    restore_cache(cache, Path::new(path))?;
                }
                return Ok(());
            }
            _ => {}
        }
    }
    Err(anyhow!(
        "usage: [cache/restore] [unreal engine root dirs...]"
    ))
}

fn hash(bytes: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn get_dependencies<P: AsRef<Path>>(path: P) -> Result<Vec<Dependency>> {
    let config = std::fs::read_to_string(path.as_ref().join(".ue4dependencies"))
        .context("could not read .ue4dependencies, is this an Unreal Engine repository?")?;
    let xml = roxmltree::Document::parse(&config).context("failed to parse .ue4dependencies")?;

    xml.descendants()
        .filter(|e| e.has_tag_name("File"))
        .map(Dependency::from)
        .collect()
}

fn restore_cache<P: AsRef<Path>>(cache: P, ue: P) -> Result<()> {
    let cache = cache.as_ref();
    let dependencies = get_dependencies(&ue)?;
    let bar = indicatif::ProgressBar::new(dependencies.len() as u64);

    use rayon::prelude::*;
    let root = PathBuf::from(ue.as_ref());
    dependencies.into_par_iter().for_each(|dep| {
        let dep_path = root.join(&dep.name);

        let restore = if cache.join(&dep_path).exists() {
            let bytes = std::fs::read(&dep_path).unwrap();
            hash(&bytes) != dep.expected_hash
        } else {
            true
        };

        if restore {
            let cache_path = cache.join(dep.expected_hash);
            if let Ok(bytes) = std::fs::read(cache_path) {
                std::fs::create_dir_all(Path::parent(&dep_path).unwrap()).unwrap();
                std::fs::write(&dep_path, bytes).unwrap();
            } else {
                bar.println(format!("missing in cache {}", dep.name));
            }
        }

        bar.inc(1);
    });
    bar.finish();
    Ok(())
}

fn build_cache<P: AsRef<Path>>(cache: P, ue: P) -> Result<()> {
    let cache = cache.as_ref();
    let dependencies = get_dependencies(&ue)?;
    let bar = indicatif::ProgressBar::new(dependencies.len() as u64);

    let root = PathBuf::from(ue.as_ref());

    let cache = |dep: &Dependency| -> Result<()> {
        if !cache.join(&dep.expected_hash).exists() {
            use sha1::{Digest, Sha1};

            let bytes = std::fs::read(root.join(&dep.name))?;
            let mut hasher = Sha1::new();
            hasher.update(&bytes);
            let hash = hex::encode(hasher.finalize());
            if hash == dep.expected_hash {
                let tmp = cache.join(format!(".{}", hash));
                std::fs::write(&tmp, &bytes)?;
                std::fs::rename(&tmp, cache.join(hash))?;
            }
        }
        Ok(())
    };

    use rayon::prelude::*;
    dependencies.into_par_iter().for_each(|dep| {
        if let Err(e) = cache(&dep) {
            bar.println(format!(
                "error caching {}: {}",
                root.join(dep.name).display(),
                e
            ));
        }
        bar.inc(1);
    });
    bar.finish();

    Ok(())
}
