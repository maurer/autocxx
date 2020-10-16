// Copyright 2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//    https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub use autocxx_engine::ParseError;
pub use autocxx_engine::Error as EngineError;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

/// Errors returned during creation of a cc::Build from an include_cxx
/// macro.
#[derive(Debug)]
pub enum Error {
    /// The cxx module couldn't parse the code generated by autocxx.
    /// This could well be a bug in autocxx.
    InvalidCxx(EngineError),
    /// The .rs file didn't exist or couldn't be parsed.
    ParseError(ParseError),
    /// We couldn't create a temporary directory to store the c++ code.
    TempDirCreationFailed(std::io::Error),
    /// We couldn't write the c++ code to disk.
    FileWriteFail(std::io::Error),
    /// No `include_cxx` macro was found anywhere.
    NoIncludeCxxMacrosFound,
    /// Problem converting the `AUTOCXX_INC` environment variable
    /// to a set of canonical paths.
    IncludeDirProblem(EngineError),
}

/// Structure for use in a build.rs file to aid with conversion
/// of a `include_cxx!` macro into a `cc::Build`.
/// This structure owns a temporary directory containing
/// the generated C++ code, as well as owning the cc::Build
/// which knows how to build it.
/// Typically you'd use this from a build.rs file by
/// using `new` and then using `builder` to fetch the `cc::Build`
/// object and asking the resultant `cc::Build` to compile the code.
/// You'll also need to set the `AUTOCXX_INC` environment variable
/// to specify the path for finding header files.
pub struct Builder {
    build: cc::Build,
    _tdir: TempDir,
}

impl Builder {
    /// Construct a Builder.
    pub fn new<P1: AsRef<Path>>(rs_file: P1, autocxx_inc: &str) -> Result<Self, Error> {
        // TODO - we have taken a different approach here from cxx.
        // cxx jumps through many (probably very justifiable) hoops
        // to generate .h and .cxx files in the Cargo out directory
        // (I think). We cheat and just make a temp dir. We shouldn't.
        let tdir = tempdir().map_err(Error::TempDirCreationFailed)?;
        let mut builder = cc::Build::new();
        builder.cpp(true);
        let autocxxes = autocxx_engine::parse_file(rs_file, Some(autocxx_inc)).map_err(Error::ParseError)?;
        let mut counter = 0;
        for include_cpp in autocxxes {
            for inc_dir in include_cpp
                .include_dirs()
                .map_err(Error::IncludeDirProblem)?
            {
                builder.include(inc_dir);
            }
            let generated_code = include_cpp
                .generate_h_and_cxx()
                .map_err(Error::InvalidCxx)?;
            for filepair in generated_code.0 {
                let fname = format!("gen{}.cxx", counter);
                counter += 1;
                let gen_cxx_path =
                    Self::write_to_file(&tdir, &fname, &filepair.implementation)
                        .map_err(Error::FileWriteFail)?;
                builder.file(gen_cxx_path);

                Self::write_to_file(&tdir, &filepair.header_name, &filepair.header)
                    .map_err(Error::FileWriteFail)?;
            }
        }
        if counter == 0 {
            Err(Error::NoIncludeCxxMacrosFound)
        } else {
            Ok(Builder {
                build: builder,
                _tdir: tdir,
            })
        }
    }

    /// Fetch the cc::Build from this.
    pub fn builder(&mut self) -> &mut cc::Build {
        &mut self.build
    }

    fn write_to_file(tdir: &TempDir, filename: &str, content: &[u8]) -> std::io::Result<PathBuf> {
        let path = tdir.path().join(filename);
        let mut f = File::create(&path)?;
        f.write_all(content)?;
        Ok(path)
    }
}
