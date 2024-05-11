use std::process::ExitCode;
use std::{io, env};

use mdbook::errors::Result as MdbookResult;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor};
use mdbook_infobox::InfoboxPreprocessor;

fn main() -> MdbookResult<ExitCode> {
    let args: Vec<_> = env::args().collect();
    if let [_, command, argument] = &args[..] {
        if command == "supports" {
            return Ok(match argument.as_str() {
                "html" => ExitCode::SUCCESS,
                _ => ExitCode::FAILURE
            });
        }
    }

    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    let processed_book = InfoboxPreprocessor.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(ExitCode::SUCCESS)
}
