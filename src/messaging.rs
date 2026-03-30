use amqprs::callbacks::{DefaultChannelCallback, DefaultConnectionCallback};
use amqprs::channel::{BasicPublishArguments, Channel, ExchangeDeclareArguments};
use amqprs::connection::{Connection, OpenConnectionArguments};
use amqprs::BasicProperties;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error, warn};

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

    pub async fn setup_command_consumer(&self, queue_name: &str, routing_key: &str, _cmd_tx: tokio::sync::mpsc::Sender<crate::model::BotCommand>) -> Result<(), Box<dyn std::error::Error>> {
        // This method would set up a consumer for commands
        // For now, we'll just log that it was called
        info!("Setting up command consumer on queue '{}' with routing key '{}'", queue_name, routing_key);
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
