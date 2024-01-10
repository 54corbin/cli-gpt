use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs},
    Client,
};
use clap::Parser;
use futures::StreamExt;
use std::{
    env,
    io::{self, stdout, BufRead, Write},
    println,
};
use termimad::crossterm::{
    cursor::MoveUp,
    queue,
    style::Color::*,
    terminal::{Clear, ClearType},
};
use termimad::*;

#[tokio::main]
async fn main() {
    let mut app = App::new();
    // app.init().await;
    app.run().await;
}

#[derive(Parser, Debug)]
struct AppArgs {
    #[arg(short = '4', long, default_value_t = false)]
    enable_gpt4: bool,

    pmt: Vec<String>,
}

struct App {
    client: Client<OpenAIConfig>,
    skin: MadSkin, // theme for rendering output messages(etc: MD, code snippet...)
    model: &'static str,
    initial_pmt: String,
}

impl App {
    //main loop
    pub async fn run(&mut self) {
        println!("Tip: two continuous enters for sending");
        if !self.initial_pmt.is_empty() {
            self.send_message(self.initial_pmt.clone()).await;
        } else {
            eprintln!(
                "{}",
                self.skin.term_text("Hello! How can I assist you today?\n")
            );
        }

        loop {
            let pmt = Self::read_pmt();
            if pmt.len() > 1 {
                self.send_message(pmt).await;
            }
        }
    }

    pub fn new() -> Self {
        let api_key = match env::var("OPENAI_API_KEY") {
            Ok(val) => {
                // println!("api key: {val:?}");
                val
            }
            Err(_) => {
                panic!("Set OPENAI_API_KEY as env var first please!");
            }
        };

        let config = OpenAIConfig::new().with_api_key(api_key);
        let client = Client::with_config(config);
        let mut skin = MadSkin::default();
        skin.set_fg(DarkCyan);

        let mut model = "gpt-3.5-turbo";
        let args = AppArgs::parse();
        if args.enable_gpt4 {
            model = "gpt-4-1106-preview";
        }

        let pmt = args.pmt.join(" ");

        Self {
            client,
            skin,
            model,
            initial_pmt: pmt,
        }
    }

    fn read_pmt() -> String {
        let mut buf = String::new();

        let lines = io::stdin().lock().lines();

        // read until blank line(two continuous enters)
        for line in lines {
            let last_line = line.unwrap();

            if last_line.len() <= 1 {
                break;
            }
            buf.push_str(&last_line);
        }

        buf
    }

    async fn send_message(&mut self, pmt: String) {
        // println!("sending... model={}", self.model);
        let messages = [ChatCompletionRequestUserMessageArgs::default()
            .content(pmt)
            .build()
            .unwrap()
            .into()];
        let request = CreateChatCompletionRequestArgs::default()
            .model(self.model)
            .max_tokens(123_u16)
            .messages(messages)
            .build()
            .unwrap();
        // println!("request: {:#?}", request);

        let mut stream = self.client.chat().create_stream(request).await.unwrap();

        // From Rust docs on print: https://doc.rust-lang.org/std/macro.print.html
        //
        //  Note that stdout is frequently line-buffered by default so it may be necessary
        //  to use io::stdout().flush() to ensure the output is emitted immediately.
        //
        //  The print! macro will lock the standard output on each call.
        //  If you call print! within a hot loop, this behavior may be the bottleneck of the loop.
        //  To avoid this, lock stdout with io::stdout().lock():
        let mut lock = stdout().lock();
        let mut resp_buf = "".to_string();
        while let Some(result) = stream.next().await {
            match result {
                Ok(resp) => resp.choices.iter().for_each(|chat_choice| {
                    if let Some(ref content) = chat_choice.delta.content {
                        write!(lock, "{content}").unwrap();
                        resp_buf.push_str(content.as_ref());
                    }
                }),
                Err(e) => {
                    writeln!(lock, "error: {:#?}", e).unwrap();
                }
            }
            stdout().flush().unwrap();
        }

        let resp_lines = resp_buf.lines().count();

        //clean the raw content and reformat the whole content from gpt
        let _ = queue!(
            stdout(),
            MoveUp(resp_lines as u16),
            Clear(ClearType::CurrentLine),
            Clear(ClearType::FromCursorDown),
            Clear(ClearType::UntilNewLine),
        );

        // format the whole content as MD
        self.skin.print_text(resp_buf.as_str());
        stdout().flush().unwrap();
        println!("\n");
    }
}
