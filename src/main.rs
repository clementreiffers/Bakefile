use clap::Parser;
use colored::*;
use duct::{cmd, ReaderHandle};
use loading::Loading;
use reqwest::Error;
use std::fmt::format;
use std::fs::File;
use std::io::{self, Read, Write};
use std::io::{BufRead, BufReader};
use std::process::exit;
use url::Url;
#[derive(Debug)]
struct Bakefile {
    variables: Vec<(String, String)>,
    rules: Vec<Rule>,
    includes: Vec<String>,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    rule: String,
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug)]
struct Rule {
    target: String,
    dependencies: Vec<String>,
    recipe: Vec<String>,
}

fn store_variable(variables: &mut Vec<(String, String)>, line: &str) {
    line.split_once('=')
        .map(|(key, value)| variables.push((key.trim().to_string(), value.trim().to_string())));
}

fn store_recipe(rules: &mut Vec<Rule>, line: &str, target: &str) {
    rules
        .iter_mut()
        .find(|rule| rule.target == target)
        .map(|rule| rule.recipe.push(line.trim().to_string()));
}

fn get_rule<'a>(bakefile: &'a Bakefile, target: &'a str) -> Option<&'a Rule> {
    bakefile.rules.iter().find(|rule| rule.target == target)
}

fn read_local_bakefile(filename: &str) -> String {
    println!("Reading bakefile from {}", filename);
    io::BufReader::new(File::open(filename).expect("File; not found"))
        .lines()
        .filter_map(Result::ok) // Ignore les erreurs de lecture
        .collect::<Vec<String>>()
        .join("\n")
}

fn clean_url(url: &str) -> String {
    if url.starts_with('"') && url.ends_with('"') || url.starts_with('\'') && url.ends_with('\'') {
        url.chars().skip(1).take(url.len() - 2).collect()
    } else {
        url.to_string()
    }
}

async fn get_hosted_bakefile(url: &str) -> String {
    let url = clean_url(url);
    if Url::parse(&url).is_ok() {
        reqwest::get(url)
            .await
            .expect("unable to get response")
            .text()
            .await
            .expect("unable to get the body")
    } else {
        panic!("Invalid URL provided {}", url);
    }
}

async fn populate_bakefile(
    filename: &str,
    variables: &mut Vec<(String, String)>,
    rules: &mut Vec<Rule>,
    includes: &mut Vec<String>,
) -> Result<(), Error> {
    let mut current_target: String = String::new();
    let mut is_including: bool = false;

    let content: String = match filename.contains("http") {
        true => get_hosted_bakefile(filename).await,
        false => read_local_bakefile(filename),
    };

    content
        .split("\n")
        .filter(|line| !line.is_empty() && !line.starts_with('#')) // Filtre les lignes vides ou commentaires
        .for_each(|line| {
            if line.starts_with(' ') || line.starts_with('\t') {
                if is_including {
                    includes.push(line.trim().to_string());
                } else {
                    store_recipe(rules, &line, &current_target);
                }
            } else if let Some((target, dependencies)) = line.split_once(':') {
                if target == "include" {
                    is_including = true;
                    return;
                }
                let target: String = target.trim().to_string();
                current_target = target.clone();
                is_including = false;
                rules.push(Rule {
                    target,
                    dependencies: dependencies.split_whitespace().map(String::from).collect(),
                    recipe: Vec::new(),
                });
            } else {
                store_variable(variables, &line);
            }
        });
    Ok(())
}
async fn read_bakefile(filename: &str) -> io::Result<Bakefile> {
    let mut variables = Vec::new();
    let mut rules = Vec::new();
    let mut includes = Vec::new();

    // Initial population of the bakefile
    populate_bakefile(filename, &mut variables, &mut rules, &mut includes)
        .await
        .expect("unable to populate bakefile");

    // Collect includes that need further processing
    while let Some(include) = includes.pop() {
        populate_bakefile(&include, &mut variables, &mut rules, &mut includes)
            .await
            .expect("unable to populate bakefile");
    }

    Ok(Bakefile {
        variables,
        rules,
        includes,
    })
}

fn execute_command(command: String, verbose: &bool) {
    let (base_cmd, args): (&str, Vec<&str>) = command
        .split_whitespace()
        .collect::<Vec<&str>>()
        .split_first()
        .map(|(first, rest)| (*first, rest.to_vec()))
        .expect("No command found");

    let loading = Loading::default();
    loading.text(command.clone());

    match cmd(base_cmd, args).stderr_to_stdout().reader() {
        Ok(result) => {
            let reader = BufReader::new(result);

            for line in reader.lines() {
                let line = line.expect("failed to read line");
                if *verbose {
                    println!("\n{}", line)
                }
            }
            loading.success(format!("{}", command));
            loading.end();
        }
        Err(e) => {
            loading.fail(format!("{}", command));
            if *verbose {
                println!("\n{}", e)
            }
            loading.end();
            exit(1);
        }
    };
}

fn execute_recipe(recipe: &[String], variables: &[(String, String)], verbose: &bool) {
    recipe
        .iter()
        .filter(|command| !command.is_empty())
        .for_each(|command| {
            execute_command(set_variables(command, variables), verbose);
        });
}

fn set_variables(command: &str, variables: &[(String, String)]) -> String {
    variables
        .iter()
        .fold(command.to_string(), |cmd, (key, value)| {
            cmd.replace(&format!("${}", key), value)
                .replace(&format!("$({})", key), value)
                .replace(&format!("${{{}}}", key), value)
        })
}

fn execute_rule(bakefile: &Bakefile, target_rule: &str, verbose: &bool) {
    if let Some(rule) = get_rule(bakefile, target_rule) {
        rule.dependencies
            .iter()
            .filter_map(|dependency| get_rule(bakefile, dependency))
            .for_each(|dependency_rule| execute_rule(bakefile, &dependency_rule.target, verbose));

        println!("{}", rule.target.bold().blue());
        execute_recipe(&rule.recipe, &bakefile.variables, &verbose);
    }
}

#[tokio::main]
async fn main() {
    let args: Args = Args::parse();
    let bakefile = read_bakefile("Bakefile").await.unwrap();

    println!("{:?}", bakefile);
    execute_rule(&bakefile, &args.rule, &args.verbose);
}
