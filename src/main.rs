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
    io::{stdout, Write},
    panic, println,
    process::exit,
    rc::Rc,
};
use termimad::crossterm::{
    cursor::{self, MoveLeft, MoveToPreviousLine},
    event::{self, Event},
    execute, queue,
    style::{self, Color::*},
    terminal::{disable_raw_mode, enable_raw_mode, size, Clear, ClearType},
    ExecutableCommand,
};
use termimad::*;

#[tokio::main]
async fn main() {
    panic::set_hook(Box::new(|_| {
        disable_raw_mode().unwrap();
    }));
    let mut app = App::new();
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
    client: Client<OpenAIConfig>,               // chatgpt's api sdk client
    skin: MadSkin, // theme for rendering output messages(etc: MD, code snippet...)
    model: &'static str, // chatgpt models.(eg: gpt-3.5-turbo, gpt-4-1106-preview)
    initial_pmt: String, // stands for initial prompt
    history: Vec<ChatCompletionRequestMessage>, // for storing the chat history
}

impl App {
    //main loop
    pub async fn run(&mut self) {
        println!("Tips: two continuous enters for sending.");
        if !self.initial_pmt.is_empty() {
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
            print!("\n------\n{:#?}", pmt);
            // if pmt.len() > 1 {
            //     if let Ok(stream) = self.send_message(pmt).await {
            //         self.streaming_and_rendering_resp(stream).await;
            //     };
            // }
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

    // read user input from terminal
    fn read_pmt() -> String {
        // with raw mode enabled, we need to handle every aspect of stdout(eg: short-cut,
        // backspace, every key stroke, etc)
        let _ = enable_raw_mode();

        // let mut pmt = String::new();
        let mut pmts: Vec<String> = Vec::new();
        let mut cursor_index: usize = 0;
        let mut pmts_index: usize = 0;
        let mut stdout = stdout();
        loop {
            if let Event::Key(key) = event::read().unwrap() {
                match key.code {
                    event::KeyCode::Enter => {
                        //ctrl-Enter for sending pmt
                        if key.modifiers.contains(event::KeyModifiers::CONTROL) {
                            let _ = disable_raw_mode();
                            println!("ctrl + enter");
                            return pmts.join("");
                        }

                        let lines = pmts.len();
                        if let Some(line) = pmts.get_mut(pmts_index) {
                            //at the last line
                            if pmts_index == lines - 1 {
                                line.push('\n');
                                pmts_index += 1;
                                pmts.insert(pmts_index, String::from("\n"));
                                execute!(stdout, cursor::MoveToColumn(1)).unwrap();
                                execute!(stdout, style::Print("\n")).unwrap();
                                cursor_index = 0;
                                continue;
                            }
                            line.insert(cursor_index, '\n');
                            let right: Rc<String> =
                                Rc::new(line.drain(1 + cursor_index..).collect());
                            pmts.insert(pmts_index + 1, Rc::clone(&right).to_string());

                            let last_row = cursor::position().unwrap().1;
                            execute!(stdout, Clear(ClearType::FromCursorDown)).unwrap();
                            let other_lines: String =
                                pmts.clone().drain(1 + pmts_index..).collect::<String>();
                            let _ = disable_raw_mode();
                            print!("\n{other_lines}");
                            let _ = enable_raw_mode();
                            let current_row = cursor::position().unwrap().1;
                            execute!(stdout, cursor::MoveUp(current_row - last_row - 1)).unwrap();
                            execute!(stdout, cursor::MoveToColumn(1)).unwrap();
                            cursor_index = 0;
                        } else {
                            pmts.insert(pmts_index, String::from("\n"));
                            execute!(stdout, style::Print("\n")).unwrap();
                        }
                        pmts_index += 1;
                    }

                    event::KeyCode::Up => {
                        if pmts_index > 0 {
                            stdout.execute(cursor::MoveUp(1)).unwrap();
                            pmts_index -= 1;

                            let current_line = pmts.get(pmts_index).unwrap();
                            if cursor_index > current_line.len() - 1 {
                                execute!(stdout, cursor::MoveToColumn(current_line.len() as u16))
                                    .unwrap();
                                cursor_index = current_line.len() - 1;
                            }
                        }
                    }

                    event::KeyCode::Down => {
                        if pmts_index < pmts.len() - 1 {
                            stdout.execute(cursor::MoveDown(1)).unwrap();
                            pmts_index += 1;

                            let current_line = pmts.get(pmts_index).unwrap();
                            if cursor_index > current_line.len() - 1 {
                                execute!(
                                    stdout,
                                    cursor::MoveToColumn(1 + current_line.len() as u16)
                                )
                                .unwrap();
                                cursor_index = current_line.len();
                            }
                        }
                    }

                    event::KeyCode::Left => {
                        if cursor_index > 0 {
                            stdout.execute(cursor::MoveLeft(1)).unwrap();
                            cursor_index -= 1;
                        }
                    }

                    event::KeyCode::Right => {
                        if let Some(current_line) = pmts.get(pmts_index) {
                            let mut cln = current_line.len();
                            if current_line.ends_with('\n') {
                                cln -= 1;
                            }
                            if cursor_index < cln {
                                stdout.execute(cursor::MoveRight(1)).unwrap();
                                cursor_index += 1;
                            }
                        }
                    }

                    event::KeyCode::Char(c) => {
                        // when control-c was pressed, terminate the program
                        if key.modifiers.contains(event::KeyModifiers::CONTROL) && c == 'c' {
                            if pmts.is_empty() {
                                // execute!(stdout, style::Print("\nBye!"));
                                let _ = disable_raw_mode();
                                println!("\nBye!");
                                exit(0);
                            } else {
                                execute!(stdout, cursor::MoveToColumn(1)).unwrap();
                                execute!(stdout, cursor::MoveUp(pmts.len() as u16 - 1)).unwrap();
                                execute!(stdout, Clear(ClearType::FromCursorDown)).unwrap();
                                execute!(stdout, cursor::MoveToColumn(1)).unwrap();

                                pmts.clear();
                                pmts_index = 0;
                                cursor_index = 0;
                                continue;
                            }
                        }
                        if key.modifiers.contains(event::KeyModifiers::CONTROL) && c == 'e' {
                            if let Some(current_line) = pmts.get(pmts_index) {
                                let mut cln = current_line.len();
                                if current_line.ends_with('\n') {
                                    cln -= 1;
                                }
                                cursor_index = cln;
                                execute!(stdout, cursor::MoveToColumn(cursor_index as u16 + 1))
                                    .unwrap();
                            }
                            continue;
                        }
                        if key.modifiers.contains(event::KeyModifiers::CONTROL) && c == 'a' {
                            execute!(stdout, cursor::MoveToColumn(1)).unwrap();
                            cursor_index = 0;
                            continue;
                        }

                        if let Some(current_line) = pmts.get_mut(pmts_index) {
                            current_line.insert(cursor_index, c);
                            cursor_index += 1;

                            let mut cln = current_line.len();
                            if current_line.ends_with('\n') {
                                cln -= 1;
                            }
                            if cursor_index != cln {
                                App::render_current_line(current_line, &mut stdout);
                                execute!(stdout, cursor::MoveRight(1_u16)).unwrap();
                            } else {
                                execute!(stdout, style::Print(c)).unwrap();
                            }
                        } else {
                            pmts.insert(pmts_index, c.to_string());
                            execute!(stdout, style::Print(c)).unwrap();
                            cursor_index += 1;
                        }
                    }
                    event::KeyCode::Backspace | event::KeyCode::Delete => {
                        if cursor_index > 0 {
                            let current_line = pmts.get_mut(pmts_index).unwrap();
                            cursor_index -= 1;
                            current_line.remove(cursor_index);
                            if !current_line.is_empty() {
                                App::render_current_line(current_line, &mut stdout);
                                execute!(stdout, cursor::MoveLeft(1_u16)).unwrap();
                            } else {
                                execute!(stdout, Clear(ClearType::CurrentLine)).unwrap();
                                execute!(stdout, cursor::MoveToColumn(1)).unwrap();
                            }
                        }
                    }
                    _ => break,
                }
            }
            let _ = stdout.flush();
        }
        let _ = disable_raw_mode();
        pmts.join("")
    }

    fn render_current_line(current_line: &str, stdout: &mut std::io::Stdout) {
        execute!(stdout, cursor::SavePosition).unwrap();
        execute!(stdout, cursor::MoveToColumn(1_u16)).unwrap();
        execute!(stdout, Clear(ClearType::CurrentLine)).unwrap();
        execute!(stdout, style::Print(current_line)).unwrap();
        execute!(stdout, cursor::RestorePosition).unwrap();
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
        self.render_resp(resp_buf.clone());
    }

    fn render_resp(&mut self, resp_buf: String) {
        // count the number of lines in the response buffer
        let screen_width = size().unwrap().0;
        let mut resp_lines = 0_u16;
        for line in resp_buf.lines() {
            resp_lines += (line.len() as u16 / screen_width) + 1;
        }

        if resp_lines < 1 {
            resp_lines = 1;
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
