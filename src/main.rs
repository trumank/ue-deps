use std::path::{PathBuf, Path};

#[derive(Debug)]
struct Dependency {
    name: String,
    hash: Option<String>,
    expected_hash: String,
}

impl Dependency {
    fn from(node: roxmltree::Node) -> Self {
        Dependency {
            name: node.attribute("Name").unwrap().to_owned(),
            hash: node.attribute("Hash").map(|s| s.to_owned()),
            expected_hash: node.attribute("ExpectedHash").unwrap().to_owned(),
        }
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    match args.len() {
        3 => {
            match args[1].as_str() {
                "cache" => {
                    return build_cache(PathBuf::from(&args[2]));
                },
                "restore" => {
                    return restore_cache(PathBuf::from(&args[2]));
                },
                _ => { }
            }
        }
        _ => { }
    }
    println!("usage: [cache/restore] [unreal engine root]")
}

fn restore_cache<P: AsRef<Path>>(ue: P) {
    let cache = PathBuf::from("../deps_cache");
    let config = std::fs::read_to_string(ue.as_ref().join(".ue4dependencies")).unwrap();
    let xml = roxmltree::Document::parse(&config).unwrap();

    let dependencies = xml.descendants().filter(|e| e.has_tag_name("File")).map(Dependency::from).collect::<Vec<_>>();
    let bar = indicatif::ProgressBar::new(dependencies.len() as u64);

    use rayon::prelude::*;
    let root = PathBuf::from(ue.as_ref());
    dependencies.into_par_iter().for_each(|dep| {
        //bar.println(&dep.name);

        let dep_path = root.join(&dep.name);

        let restore = if cache.join(&dep_path).exists() {
            use crypto::digest::Digest;
            use crypto::sha1::Sha1;

            let bytes = std::fs::read(&dep_path).unwrap();
            let mut hasher = Sha1::new();
            hasher.input(&bytes);
            let hash = hasher.result_str();
            hash != dep.expected_hash
        } else {
            true
        };

        if restore {
            let cache_path = cache.join(dep.expected_hash);
            if let Ok(bytes) = std::fs::read(&cache_path) {
                std::fs::create_dir_all(Path::parent(&dep_path).unwrap()).unwrap();
                std::fs::write(&dep_path, &bytes).unwrap();
            } else {
                bar.println(format!("missing in cache {}", dep.name));
            }
        }

        bar.inc(1);
    });
    bar.finish();
}

fn build_cache<P: AsRef<Path>>(ue: P) {
    let cache = PathBuf::from("../deps_cache");
    let config = std::fs::read_to_string(ue.as_ref().join(".ue4dependencies")).unwrap();
    let xml = roxmltree::Document::parse(&config).unwrap();

    let dependencies = xml.descendants().filter(|e| e.has_tag_name("File")).map(Dependency::from).collect::<Vec<_>>();
    let bar = indicatif::ProgressBar::new(dependencies.len() as u64);

    use rayon::prelude::*;
    let root = PathBuf::from(ue.as_ref());
    dependencies.into_par_iter().for_each(|dep| {
        //bar.println(&dep.name);

        if !cache.join(&dep.expected_hash).exists() {
            use crypto::digest::Digest;
            use crypto::sha1::Sha1;

            let bytes = std::fs::read(root.join(dep.name)).unwrap();
            let mut hasher = Sha1::new();
            hasher.input(&bytes);
            let hash = hasher.result_str();
            if hash == dep.expected_hash {
                let tmp = cache.join(format!(".{}", hash));
                std::fs::write(&tmp, &bytes).unwrap();
                std::fs::rename(&tmp, cache.join(hash)).unwrap();
            }
        }
        bar.inc(1);
    });
    bar.finish();
}
