use async_openai::{
    config::OpenAIConfig,
    types::{ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs},
    Client,
};
use futures::StreamExt;
use std::{
    env,
    io::{self, stdout, BufRead, Write},
    println,
};
use termimad::crossterm::style::Color::*;
use termimad::*;

#[tokio::main]
async fn main() {
    let mut app = App::new();
    println!("Hint: two continuous enters for sending");
    app.init().await;
    app.run().await;
}

struct App {
    client: Client<OpenAIConfig>,
    skin: MadSkin, // theme for rendering output messages(etc: MD, code snippet...)
}

impl App {
    //main loop
    pub async fn run(&mut self) {
        loop {
            let pmt = Self::read_pmt();
            if pmt.len() > 1 {
                self.send_message(pmt).await;
            }
        }
    }

    async fn init(&mut self) {
        match env::args()
            .skip(1) //the first cmd parm is the executeable itself
            .reduce(|p, s| format!("{:#1?} {:#2?}", p, s))
        {
            Some(pmt) => {
                self.send_message(pmt).await;
            }
            None => {
                eprintln!(
                    "{}",
                    self.skin.term_text("Hello! How can I assist you today?\n")
                );
            }
        };
    }

    pub fn new() -> Self {
        let api_key = match env::var("OPENAI_API_KEY") {
            Ok(val) => {
                println!("api key: {val:?}");
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

        Self { client, skin }
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
            // buf.push('\n');
            buf.push_str(&last_line);
        }
        buf
    }

    async fn send_message(&mut self, pmt: String) {
        // println!("sending...");
        let messages = [ChatCompletionRequestUserMessageArgs::default()
            .content(pmt.as_str())
            .build()
            .unwrap()
            .into()];
        let request = CreateChatCompletionRequestArgs::default()
            .model("gpt-3.5-turbo")
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
        let mut buf = "".to_string();
        while let Some(result) = stream.next().await {
            match result {
                Ok(resp) => resp.choices.iter().for_each(|chat_choice| {
                    if let Some(ref content) = chat_choice.delta.content {
                        write!(lock, "{content}").unwrap();
                        buf.push_str(content.as_ref());
                    }
                }),
                Err(e) => {
                    writeln!(lock, "error: {:#?}", e).unwrap();
                }
            }
            stdout().flush().unwrap();
        }
        println!("\n");

        // let buf_line_count = buf.split('\n').count() + 1;
        // println!("\nbuf len = {}, lines= {}\n", buf.len(), buf_line_count);
        //
        //clean the raw content and reformat the full content from gpt
        // for _ in 0..buf_line_count {
        //     // cursor back to the start of the line
        //     print!("\x1B[1A");
        //
        //     // clean the whole line
        //     print!("\x1B[2K");
        // }
        //
        // stdout().flush().unwrap();
        //
        // // format the whole content
        // // self.skin.term_text(buf.as_str());
        // print_text(buf.as_str());
    }
}
