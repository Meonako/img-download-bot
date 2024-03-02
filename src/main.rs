use poise::serenity_prelude as serenity;
use serenity::futures::StreamExt;

struct Data {} // User data, which is stored and accessible in all command invocations
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

const OUTPUT_DIR: &str = "outputs";

/// Download all images from this or specify channel
#[poise::command(slash_command)]
async fn download(
    ctx: Context<'_>,
    #[description = "Channel to download attachments"] channel: Option<serenity::ChannelId>,
) -> Result<(), Error> {
    let reply = ctx
        .send(
            poise::CreateReply::default()
                .content("Fetching...")
                .ephemeral(true),
        )
        .await?;

    let channel = if let Some(c) = channel {
        c
    } else {
        ctx.channel_id()
    };

    let client = reqwest::Client::new();

    let mut tasks = vec![];
    let mut messages = channel.messages_iter(&ctx).boxed();
    while let Some(message_result) = messages.next().await {
        match message_result {
            Ok(m) => {
                if m.attachments.is_empty() {
                    continue;
                }

                let client = client.clone();

                tasks.push(tokio::spawn(async move {
                    for attachment in m.attachments {
                        let response = client.get(&attachment.url).send().await;

                        if response.is_err() {
                            continue;
                        }

                        let response = response.unwrap();

                        let filename = {
                            let iter = attachment.url.split('/').collect::<Vec<_>>();
                            let message_id = iter[iter.len() - 2];
                            let og_filename = iter[iter.len() - 1];

                            let mut filename = format!(
                                "{OUTPUT_DIR}/{message_id}_{}",
                                &og_filename[0..og_filename.find('?').unwrap()].to_string()
                            );
                            let mut i = 0;

                            while std::path::Path::new(&format!("{filename}.png")).exists() {
                                filename = format!("{filename} ({i})");
                                i += 1;
                            }

                            filename
                        };

                        let bytes_result = response.bytes().await;
                        if bytes_result.is_err() {
                            continue;
                        }

                        let bytes = bytes_result.unwrap();

                        _ = std::fs::write(&filename, bytes);
                    }
                }));
            }
            Err(e) => eprintln!("{:?}", e),
        }
    }

    for task in tasks {
        task.await.unwrap();
    }

    reply
        .edit(ctx, poise::CreateReply::default().content("Finished."))
        .await?;

    println!("Finished.");

    Ok(())
}

#[tokio::main]
async fn main() {
    let token = {
        let mut args = std::env::args().collect::<Vec<_>>();
        if args.len() > 1 {
            args.remove(1)
        } else {
            std::env::var("LITTLE_KITTY").expect("missing LITTLE_KITTY")
        }
    };
    let intents = serenity::GatewayIntents::all();

    _ = std::fs::create_dir(OUTPUT_DIR);

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![download()],
            ..Default::default()
        })
        .setup(|ctx, ready, framework| {
            println!("Logged in as: {}", ready.user.name);

            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}
