use anyhow::{Context, Result};
use clap::Parser;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::path::PathBuf;
use std::io::{self, Write};





#[derive(Parser, Debug)]

struct Args {

    // replace requirementst.txt with a toml

    #[arg(short, long, default_value = "deps.toml")]
    output_file: String,
}


#[derive(Debug, Deserialize)]

struct ToolsList {
    tools: Vec<Tool>,
}


#[derive(Debug, Deserialize)]

struct Tool {
    name: String,
    version: String,
}


#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]

struct Response {
    info: PkgMetadata,
    releases: HashMap<String, Vec<ReleaseData>>,
}


#[derive(Debug, Deserialize, Clone)]

struct PkgMetadata {
    version: String,
    // needs_dist: Bool,
    #[serde(rename = "requires_dist")]
    requires_dist: Option<Vec<String>>,

}

#[allow(dead_code)]
#[derive(Debug, Deserialize, Clone)]

struct ReleaseData {
    requires_dist: Option<Vec<String>>,
}

#[derive(Debug, PartialEq, Clone)]

struct Dep {
    name: String,
    version: String,
}



impl Dep {
    fn new_dep(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
        }
    }

    fn does_need_dist(req: &str) -> Option<Self> {
        let bits: Vec<&str> = req.split(';')
            .next()?
            .split(' ')
            .collect();
        // bits as in slices of the string
        let pkg_version = bits.first()?;

        let version_identifiers = ["==", ">=", "<=", "~=", "!=", ">", "<"];

        for &identifier in &version_identifiers {
            if let Some(pos) = pkg_version.find(identifier) {
                let pkg = &pkg_version[..pos].to_string();
                let version = &pkg_version[pos + identifier.len()..].trim_matches(|c| c == '"' || c == '\'').to_string();
                return Some(Dep::new_dep(pkg, version));
            }
        }

        // fn to_txt_str(&self) -> String {
        //     format!("{}=={}", self.name, self.version)
        // }

        Some(Dep::new_dep(pkg_version.to_string(), "latest"))
    }

    fn to_txt_str(&self) -> String {
        format!("{}=={}", self.name, self.version)
    }
}


fn find_tools_json() -> io::Result<PathBuf> {
    let mut current_dir = std::env::current_dir()?;

    loop {
        let file_path = current_dir.join("pkgs.json");
        if file_path.exists() {
            return Ok(file_path);
        }

        if !current_dir.pop() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "pkgs.json not found in current directory or any parent directories",
            ));
        }
    }
}


async fn fetch_info(pkg: &str) -> Result<Response> {
    // init the reqwest client
    let client = Client::new();

    let url = format!("https://pypi.org/pypi/{}/json", pkg);

    let response = client.get(&url).send()
        .await
        .context("Could not fetch from PyPi")?;

    if !response.status().is_success() {
        anyhow::bail!("Package not found on PyPi: {}", response.status());


    }

    let pkg_data = response.json()
        .await
        .context("Could not parse PyPi response")?;

    Ok(pkg_data)


}


fn parse_deps(info: &Response) -> Vec<Dep> {
    // return type Vec not struct and match signature type
    let mut deps = Vec::new();
    // Response struct takes metadata and releases
    if let Some(requires_dist) = &info.info.requires_dist {
        for r in requires_dist {
            if let Some(dep) = Dep::does_need_dist(r) {
                if !r.contains("extra == ") && !r.contains("python_version") {
                    deps.push(dep);
                }
            }
        }
    }

    deps
}


async fn write_toml(tools: &[Tool], all_deps: &[(String, Vec<Dep>)], output:&str) -> Result<()> {
    let mut filepath = File::create(output).context("Could not create deps.toml file")?;


    for tool in tools {
        // writed toml header
        let header = format!("[{}]\n", tool.name);
        filepath.write_all(header.as_bytes())?;

        // writes pkg versions
        let version_line = format!("version = \"{}\"\n", tool.version);
        filepath.write_all(version_line.as_bytes())?;

        // look for all the dependencies then writer
        if let Some((_, deps)) = all_deps.iter().find(|(name, _)| name == &tool.name) {

            filepath.write_all(b"dependencies = [\n")?;

            for dep in deps {
                let dep_line = format!("    \"{}\",\n", dep.to_txt_str());
                filepath.write_all(dep_line.as_bytes())?;
            }

            filepath.write_all(b"]\n")?;
        }

        filepath.write_all(b"\n")?; // line breaks for clarity
    }

    Ok(())

}

#[tokio::main]

async fn main() -> Result<()> {
    let args = Args::parse();

    println!("================================================");
    println!("================================================");

    let tools_path = find_tools_json().context("Failed to find tools.json")?;
    let tools_file = File::open(tools_path).context("Failed to open tools.json")?;

    let tools_list: ToolsList = serde_json::from_reader(tools_file)
        .context("Failed to parse tools.json")?;

    let mut all_deps = Vec::new();


    for tool in &tools_list.tools {
        println!("\nFetching dependencies for {} v{}...", tool.name, tool.version);

        match fetch_info(&tool.name).await {
            Ok(data) => {
                let deps = parse_deps(&data);
                println!("Found {} dependencies", deps.len());

                all_deps.push((tool.name.clone(), deps));
            }
            Err(e) => {
                println!("Error fetching dependencies for {}: {}", tool.name, e);
                continue;
            }
        }
    }


    write_toml(&tools_list.tools, &all_deps, &args.output_file).await?;

    println!("\nAll dependencies written to {}", args.output_file);
    println!("================================================");
    println!("================================================");

    Ok(())
}
