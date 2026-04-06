use amqprs::callbacks::{DefaultChannelCallback, DefaultConnectionCallback};
use amqprs::channel::{BasicPublishArguments, BasicConsumeArguments, BasicAckArguments, Channel, ExchangeDeclareArguments, QueueDeclareArguments, QueueBindArguments};
use amqprs::connection::{Connection, OpenConnectionArguments};
use amqprs::consumer::AsyncConsumer;
use amqprs::{BasicProperties, Deliver};
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error, warn};

/// Receives raw AMQP messages, deserialises them as BotCommand, and forwards to the aggregator.
struct CommandForwarder {
    cmd_tx: tokio::sync::mpsc::Sender<crate::model::BotCommand>,
}

#[async_trait]
impl AsyncConsumer for CommandForwarder {
    async fn consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        _props: BasicProperties,
        content: Vec<u8>,
    ) {
        match serde_json::from_slice::<crate::model::BotCommand>(&content) {
            Ok(cmd) => {
                if self.cmd_tx.send(cmd).await.is_err() {
                    error!("Command channel closed — aggregator may have stopped.");
                }
            }
            Err(e) => {
                warn!("Unrecognised bot command ({}): {}", e, String::from_utf8_lossy(&content));
            }
        }
        let _ = channel
            .basic_ack(BasicAckArguments::new(deliver.delivery_tag(), false))
            .await;
    }
}

pub struct RabbitMQClient {
    _connection: Connection,
    channel: Channel,
    url: String,
}

impl RabbitMQClient {
    pub async fn new(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let args = OpenConnectionArguments::try_from(url)?;
        let connection = Connection::open(&args).await?;
        connection.register_callback(DefaultConnectionCallback).await?;

        let channel = connection.open_channel(None).await?;
        channel.register_callback(DefaultChannelCallback).await?;

        channel.exchange_declare(
            ExchangeDeclareArguments::new("arbit_hub", "topic")
                .durable(true)
                .auto_delete(false)
                .finish(),
        ).await?;

        Ok(Self { _connection: connection, channel, url: url.to_string() })
    }

    pub fn is_open(&self) -> bool {
        self._connection.is_open() && self.channel.is_open()
    }

    pub async fn reconnect(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        warn!("RabbitMQ: attempting reconnect...");
        let args = OpenConnectionArguments::try_from(self.url.as_str())?;
        let connection = Connection::open(&args).await?;
        connection.register_callback(DefaultConnectionCallback).await?;

        let channel = connection.open_channel(None).await?;
        channel.register_callback(DefaultChannelCallback).await?;

        channel.exchange_declare(
            ExchangeDeclareArguments::new("arbit_hub", "topic")
                .durable(true)
                .auto_delete(false)
                .finish(),
        ).await?;

        self._connection = connection;
        self.channel = channel;
        Ok(())
    }

    pub async fn publish<T: Serialize>(&mut self, routing_key: &str, payload: &T) -> Result<(), Box<dyn std::error::Error>> {
        if !self.is_open() {
            self.reconnect().await?;
        }
        let body = serde_json::to_vec(payload)?;
        let args = BasicPublishArguments::new("arbit_hub", routing_key);
        self.channel.basic_publish(BasicProperties::default(), body, args).await?;
        Ok(())
    }

    pub async fn setup_command_consumer(
        &self,
        queue_name: &str,
        routing_key: &str,
        cmd_tx: tokio::sync::mpsc::Sender<crate::model::BotCommand>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Declare durable queue so it survives broker restarts
        let (q, _, _) = self.channel
            .queue_declare(QueueDeclareArguments::new(queue_name).durable(true).finish())
            .await?
            .ok_or("queue_declare returned None")?;

        // Bind to the topic exchange
        self.channel
            .queue_bind(QueueBindArguments::new(&q, "arbit_hub", routing_key))
            .await?;

        // Register async consumer — messages are forwarded to the aggregator via cmd_tx
        self.channel
            .basic_consume(
                CommandForwarder { cmd_tx },
                BasicConsumeArguments::new(&q, "arbit-bot-cmd-consumer"),
            )
            .await?;

        info!("Command consumer active on queue='{}' routing_key='{}'", queue_name, routing_key);
        Ok(())
    }

    pub async fn consume(&self, queue_name: &str, routing_key: &str, _consumer: crate::aggregator::CommandConsumer) -> Result<(), Box<dyn std::error::Error>> {
        // This method would set up a consumer
        // For now, we'll just log that it was called
        info!("Setting up consumer on queue '{}' with routing key '{}'", queue_name, routing_key);
        Ok(())
    }
}

pub type SharedMessaging = Arc<Mutex<Option<RabbitMQClient>>>;

pub async fn init_messaging() -> SharedMessaging {
    let url = std::env::var("RABBITMQ_URL").unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/".to_string());
    
    let mut retry_count = 0;
    let max_retries = 30;
    
    while retry_count < max_retries {
        match RabbitMQClient::new(&url).await {
            Ok(client) => {
                info!("Connected to RabbitMQ at {}", url);
                return Arc::new(Mutex::new(Some(client)));
            }
            Err(e) => {
                retry_count += 1;
                let delay = std::cmp::min(2u64.pow(retry_count.min(5)), 30);
                error!("Failed to connect to RabbitMQ (attempt {}/{}): {}. Retrying in {}s...", retry_count, max_retries, e, delay);
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;
            }
        }
    }

    error!("Failed to connect to RabbitMQ after {} attempts. Messaging will be disabled.", max_retries);
    Arc::new(Mutex::new(None))
}
