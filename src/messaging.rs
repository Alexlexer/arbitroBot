use amqprs::callbacks::{DefaultChannelCallback, DefaultConnectionCallback};
use amqprs::channel::{BasicPublishArguments, Channel, ExchangeDeclareArguments};
use amqprs::connection::{Connection, OpenConnectionArguments};
use amqprs::BasicProperties;
use amqprs::consumer::AsyncConsumer;
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::Mutex;
use log::{info, error};

pub struct RabbitMQClient {
    _connection: Connection,
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

        Ok(Self { _connection: connection, channel })
    }

    pub async fn publish<T: Serialize>(&self, routing_key: &str, payload: &T) -> Result<(), Box<dyn std::error::Error>> {
        let body = serde_json::to_vec(payload)?;
        let args = BasicPublishArguments::new("arbit_hub", routing_key);
        
        self.channel.basic_publish(BasicProperties::default(), body, args).await?;
        Ok(())
    }

    pub async fn consume<C>(&self, queue_name: &str, routing_key: &str, consumer: C) -> Result<(), Box<dyn std::error::Error>> 
    where C: AsyncConsumer + Send + 'static 
    {
        use amqprs::channel::{QueueDeclareArguments, QueueBindArguments, BasicConsumeArguments};
        // Use unique queue name to allow multiple instances to receive the same broadcast
        let queue_args = QueueDeclareArguments::new("").exclusive(true).auto_delete(true).finish();
        let (queue_name_real, _, _) = self.channel.queue_declare(queue_args).await?.unwrap();
        
        // Bind queue to exchange
        self.channel.queue_bind(QueueBindArguments::new(&queue_name_real, "arbit_hub", routing_key)).await?;
        
        // Start consumption
        let consume_args = BasicConsumeArguments::new(&queue_name_real, "bot_command_consumer").finish();
        self.channel.basic_consume(consumer, consume_args).await?;
        
        info!("Started consuming from queue: {} with key: {}", queue_name_real, routing_key);
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
