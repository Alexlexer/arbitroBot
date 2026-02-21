use amqprs::callbacks::{DefaultChannelCallback, DefaultConnectionCallback};
use amqprs::channel::{BasicPublishArguments, Channel, ExchangeDeclareArguments};
use amqprs::connection::{Connection, OpenConnectionArguments};
use amqprs::BasicProperties;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error};

pub struct RabbitMQClient {
    connection: Connection,
    channel: Channel,
}

impl RabbitMQClient {
    pub async fn new(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let args = OpenConnectionArguments::try_from(url)?;
        let connection = Connection::open(&args).await?;
        connection.register_callback(DefaultConnectionCallback).await?;

        let channel = connection.open_channel(None).await?;
        channel.register_callback(DefaultChannelCallback).await?;

        // Declare main exchange
        channel.exchange_declare(ExchangeDeclareArguments::new("arbit_hub", "topic")).await?;

        Ok(Self { connection, channel })
    }

    pub async fn publish<T: Serialize>(&self, routing_key: &str, payload: &T) -> Result<(), Box<dyn std::error::Error>> {
        let body = serde_json::to_vec(payload)?;
        let args = BasicPublishArguments::new("arbit_hub", routing_key);
        
        self.channel.basic_publish(BasicProperties::default(), body, args).await?;
        Ok(())
    }
}

pub type SharedMessaging = Arc<Mutex<Option<RabbitMQClient>>>;

pub async fn init_messaging() -> SharedMessaging {
    let url = std::env::var("RABBITMQ_URL").unwrap_or_else(|_| "amqp://guest:guest@localhost:5672/".to_string());
    match RabbitMQClient::new(&url).await {
        Ok(client) => {
            info!("Connected to RabbitMQ at {}", url);
            Arc::new(Mutex::new(Some(client)))
        }
        Err(e) => {
            error!("Failed to connect to RabbitMQ: {}. Messaging will be disabled.", e);
            Arc::new(Mutex::new(None))
        }
    }
}
