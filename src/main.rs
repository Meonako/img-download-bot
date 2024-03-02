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

    let mut tasks = vec![];
    let mut messages = channel.messages_iter(&ctx).boxed();
    while let Some(message_result) = messages.next().await {
        match message_result {
            Ok(m) => {
                if m.attachments.is_empty() {
                    continue;
                }

                tasks.push(tokio::spawn(async move {
                    for attachment in m.attachments {
                        let filename = format!(
                            "{OUTPUT_DIR}/{}_{}",
                            attachment.id.get(),
                            attachment.filename
                        );

                        let download_result = attachment.download().await;
                        match download_result {
                            Ok(b) => {
                                if let Err(e) = std::fs::write(filename, b) {
                                    eprintln!("Save error: {e}");
                                }
                            }
                            Err(e) => {
                                eprintln!("Download error: {e}");
                            }
                        }
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
