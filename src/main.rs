use async_openai::{
    config::OpenAIConfig,
    error::OpenAIError,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestUserMessageArgs, ChatCompletionResponseStream,
        CreateChatCompletionRequestArgs,
    },
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
    cursor::{MoveLeft, MoveToPreviousLine},
    queue,
    style::Color::*,
    terminal::{size, Clear, ClearType},
};
use termimad::*;

#[tokio::main]
async fn main() {
    let mut app = App::new();
    // app.init().await;
    app.run().await;
}

// args for the app, can be passed in from the command line
#[derive(Parser, Debug)]
struct AppArgs {
    #[arg(short = '4', long, default_value_t = false)]
    enable_gpt4: bool,

    pmt: Vec<String>,
}

struct App {
    client: Client<OpenAIConfig>, // chatgpt's api sdk client
    skin: MadSkin,                // theme for rendering output messages(etc: MD, code snippet...)
    model: &'static str,          // stands for different chatgpt models.
    // eg: gpt-3.5-turbo, gpt-4-1106-preview
    initial_pmt: String,                        // stands for initial prompt
    history: Vec<ChatCompletionRequestMessage>, // for storing the chat history
}

impl App {
    //main loop
    pub async fn run(&mut self) {
        println!("Tip: two continuous enters for sending");
        if !self.initial_pmt.is_empty() {
            // self.send_message(self.initial_pmt.clone()).await;
            if let Ok(stream) = self.send_message(self.initial_pmt.clone()).await {
                self.streaming_and_rendering_resp(stream).await;
            };
        } else {
            eprintln!(
                "{}",
                self.skin.term_text("Hello! How can I assist you today?\n")
            );
        }

        loop {
            let pmt = Self::read_pmt();
            if pmt.len() > 1 {
                if let Ok(stream) = self.send_message(pmt).await {
                    self.streaming_and_rendering_resp(stream).await;
                };
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
            history: Vec::new(),
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

    async fn send_message(
        &mut self,
        pmt: String,
    ) -> Result<ChatCompletionResponseStream, OpenAIError> {
        // println!(
        //     "sending... model={} \t history count={}",
        //     self.model,
        //     self.history.len()
        // );
        let message = ChatCompletionRequestUserMessageArgs::default()
            .content(pmt)
            .build()
            .unwrap()
            .into();

        self.history.push(message);
        let request = CreateChatCompletionRequestArgs::default()
            .model(self.model)
            .max_tokens(1234_u16)
            .messages(self.history.to_vec())
            .build()
            .unwrap();
        // println!("request: {:#?}", request);

        self.client.chat().create_stream(request).await
    }

    //read response from the stream and print it as markdown
    async fn streaming_and_rendering_resp(&mut self, mut stream: ChatCompletionResponseStream) {
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

        let resp = ChatCompletionRequestAssistantMessageArgs::default()
            .content(resp_buf.clone())
            .build()
            .unwrap();
        self.history.push(resp.into());

        // count the number of lines in the response buffer
        let screen_width = size().unwrap().0;
        let mut resp_lines = 0_u16;
        for line in resp_buf.lines() {
            resp_lines += (line.len() as u16 / screen_width) + 1;
        }

        //clean the raw content and reformat the whole content from gpt
        let _ = queue!(
            stdout(),
            MoveToPreviousLine(resp_lines - 1),
            MoveLeft(screen_width),
            Clear(ClearType::FromCursorDown),
        );

        // format the whole content as MD
        self.skin.print_text(resp_buf.as_str());
        stdout().flush().unwrap();
        println!("\n");
        // println!("response lines: {resp_lines} \t screen width: {screen_width}");
    }
}
