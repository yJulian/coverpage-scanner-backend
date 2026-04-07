use image::DynamicImage;
use crate::models::PartialStudentInfo;
use anyhow::Result;

pub mod steps;

pub struct Context {
    pub image: DynamicImage,
    pub partial_info: PartialStudentInfo,
    pub qr_found: bool,
}

impl Context {
    pub fn new(image: DynamicImage) -> Self {
        Self {
            image,
            partial_info: PartialStudentInfo::default(),
            qr_found: false,
        }
    }

    pub fn is_complete(&self) -> bool {
        self.partial_info.first_name.is_some()
            && self.partial_info.last_name.is_some()
            && self.partial_info.matriculation_number.is_some()
    }

    pub fn get_missing_fields(&self) -> Vec<String> {
        let mut missing = Vec::new();
        if self.partial_info.first_name.is_none() {
            missing.push("first_name".to_string());
        }
        if self.partial_info.last_name.is_none() {
            missing.push("last_name".to_string());
        }
        if self.partial_info.matriculation_number.is_none() {
            missing.push("matriculation_number".to_string());
        }
        missing
    }
}

pub trait ProcessingStep: Send + Sync {
    fn name(&self) -> &str;
    fn process(&self, context: &mut Context) -> Result<()>;
}

pub struct ScannerPipeline {
    steps: Vec<Box<dyn ProcessingStep>>,
}

impl ScannerPipeline {
    pub fn new() -> Self {
        Self { steps: Vec::new() }
    }

    pub fn add_step(mut self, step: Box<dyn ProcessingStep>) -> Self {
        self.steps.push(step);
        self
    }

    pub fn run(&self, mut context: Context) -> Result<Context> {
        for step in &self.steps {
            tracing::info!("Running step: {}", step.name());
            step.process(&mut context)?;
            
            // Check early exit
            if context.qr_found && context.is_complete() {
                tracing::info!("QR code found and info complete, skipping further steps.");
                break;
            }
        }
        Ok(context)
    }
}
