use std::collections::HashMap;
use std::fs;
use std::io;
use std::path;
use std::sync::Arc;

mod params;

use params::Parameter;
pub use params::{Error as ParamError, QuizParameters};

pub(crate) struct LoadedConfigs {
    // name, config
    pub success: HashMap<Box<str>, QuizConfig>,
    // name, path
    pub duplicates: Vec<(Box<str>, String)>,
    // error, path
    pub errors: Vec<(LoadConfigError, String)>,
}

pub struct LoadedConfigsResult {
    pub num_configs: usize,
    pub num_dupes: usize,
    pub errors: Vec<(LoadConfigError, String)>,
}

#[derive(thiserror::Error, Debug)]
pub enum LoadConfigError {
    #[error("failed to deserialize quiz query from yaml file: {0}")]
    DeserializeErr(serde_yaml::Error),
    #[error("failed to convert to quiz config : {0}")]
    ConvertErr(Error),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to create quiz query directory: {0}")]
    CreateDirErr(io::Error),
    #[error("failed to get filepath for quiz query file: {0}")]
    FilepathError(glob::GlobError),
    #[error("error reading file for query ({1}): {0}")]
    ReadFileError(io::Error, String),
    #[error("SQL query with no limit clause detected")]
    QueryWithNoLimit,
}

// TODO: tidy this up, return a better type
pub(crate) fn read_configs() -> Result<LoadedConfigs, Error> {
    // Default to working directory
    let data_dir = std::env::var("CHOKOTAN_DATA_PATH").unwrap_or_else(|_| String::from("."));
    let path = format!("{}/chokotan/quiz/configs/*.yaml", &data_dir);
    if let Some(p) = AsRef::<path::Path>::as_ref(&path).parent() {
        fs::create_dir_all(p).map_err(Error::CreateDirErr)?;
    }

    let query_dir = format!("{}/chokotan/quiz/queries/", &data_dir);

    let mut configs = HashMap::new();
    let mut duplicates = Vec::new();
    let mut errors = Vec::new();
    for filepath in glob::glob(&path).expect("quiz query glob pattern error") {
        let filepath = filepath.map_err(Error::FilepathError)?;
        let path = filepath
            .strip_prefix(&data_dir)
            .expect("path prefix does not match")
            .to_string_lossy();
        let file =
            fs::File::open(&filepath).map_err(|e| Error::ReadFileError(e, path.to_string()))?;

        match load_config(file, &query_dir) {
            Ok(v) => {
                if let Some(old) = configs.insert(v.name.clone(), v) {
                    duplicates.push((old.name, path.to_string()));
                }
            }
            Err(e) => {
                errors.push((e, path.to_string()));
            }
        }
    }
    log::info!(
        "Read {} quiz query configurations with {} errors.",
        configs.len(),
        errors.len()
    );

    Ok(LoadedConfigs {
        success: configs,
        duplicates,
        errors,
    })
}

fn load_config(file: fs::File, query_dir: &str) -> Result<QuizConfig, LoadConfigError> {
    let reader = io::BufReader::new(file);
    let result = serde_yaml::from_reader::<_, QuizConfigYaml>(reader);
    let builder = result.map_err(LoadConfigError::DeserializeErr)?;
    let config = builder
        .to_config(query_dir)
        .map_err(LoadConfigError::ConvertErr)?;
    Ok(config)
}

#[derive(serde::Deserialize)]
struct QuizConfigYaml {
    // name of the query
    name: Box<str>,
    description: Option<Box<str>>,
    // filename of the query
    #[serde(rename = "query")]
    query_file: String,
    // map of parameter names and types that can be passed in
    #[serde(default)]
    params: Vec<Parameter>,
    // map of columns returned by the query to types to parse as
    #[serde(default)]
    types: HashMap<Box<str>, database::ValueType>,
    // list of fields to display in the result
    fields: Arc<[QuizInfoField]>,
}

impl QuizConfigYaml {
    fn to_config(self, query_dir: &str) -> Result<QuizConfig, Error> {
        let filepath = format!("{}/{}", query_dir, &self.query_file);
        // Read the actual SQL query itself
        let query = std::fs::read_to_string(filepath)
            .map_err(|e| Error::ReadFileError(e, self.query_file))?;
        // TODO: Ideally I would want to be able to parse the query here
        //  and identify/add a limit clause to a parameter afterwards
        // However, libpg_query does not compile on Windows, so this is not great
        // Therefore for now, let's just throw an error if there is no limit
        //  and strust the query writer
        if !(query.contains("LIMIT ") || query.contains("limit ")) {
            return Err(Error::QueryWithNoLimit);
        }

        Ok(QuizConfig {
            name: self.name,
            description: self.description,
            query: query.into(),
            params: self.params,
            types: self.types,
            fields: self.fields,
        })
    }
}

pub(crate) struct QuizConfig {
    // name of the query
    name: Box<str>,
    description: Option<Box<str>>,
    // text of the sql query to use
    query: Box<str>,
    // map of parameter names and types that can be passed in
    // TODO: validate default is correct and transform
    params: Vec<Parameter>,
    // map of columns returned by the query to types to parse as
    types: HashMap<Box<str>, database::ValueType>,
    // list of fields to display in the result
    // TODO: validate each field actually exists as a column
    fields: Arc<[QuizInfoField]>,
}

impl QuizConfig {
    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub(crate) fn query(&self) -> &str {
        &self.query
    }

    pub(crate) fn db_types(&self) -> &HashMap<Box<str>, database::ValueType> {
        &self.types
    }

    pub(crate) fn fields(&self) -> Arc<[QuizInfoField]> {
        self.fields.clone()
    }
}

#[derive(serde::Deserialize)]
pub struct QuizInfoField {
    // name to display
    pub name: Box<str>,
    // column get from in the query
    pub col: Box<str>,
    // type of the column
    #[serde(rename = "kind")]
    pub _kind: Option<QuizInfoFieldType>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QuizInfoFieldType {
    String,
}
