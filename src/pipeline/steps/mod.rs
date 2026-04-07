use crate::models::{PartialStudentInfo, StudentInfo};
use crate::ocr::OcrProvider;
use crate::pipeline::{Context, ProcessingStep};
use anyhow::Result;
use rqrr::PreparedImage;
use std::sync::Arc;

pub struct ImagePreProcessor;

impl ProcessingStep for ImagePreProcessor {
    fn name(&self) -> &str {
        "ImagePreProcessor"
    }

    fn process(&self, context: &mut Context) -> Result<()> {
        tracing::info!("Optimizing image...");
        // Convert to grayscale
        let gray = context.image.grayscale();
        // Maybe increase contrast
        // For simplicity, we just store the grayscaled image back
        context.image = gray;
        Ok(())
    }
}

pub struct QrCodeScanner;

impl ProcessingStep for QrCodeScanner {
    fn name(&self) -> &str {
        "QrCodeScanner"
    }

    fn process(&self, context: &mut Context) -> Result<()> {
        tracing::info!("Searching for QR code...");
        let img = context.image.to_luma8();
        let mut prepared = PreparedImage::prepare(img);
        let grids = prepared.detect_grids();

        for grid in grids {
            let (_meta, content) = grid.decode()?;
            tracing::info!("Found QR code: {}", content);

            // Assume content is JSON for student info or CSV
            // Let's try to parse as JSON first
            if let Ok(info) = serde_json::from_str::<StudentInfo>(&content) {
                context.partial_info.first_name = Some(info.first_name);
                context.partial_info.last_name = Some(info.last_name);
                context.partial_info.matriculation_number = Some(info.matriculation_number);
                context.qr_found = true;
                return Ok(());
            }
        }

        tracing::info!("No QR code found or invalid format.");
        Ok(())
    }
}

pub struct OcrScanner {
    pub provider: Arc<dyn OcrProvider>,
}

impl ProcessingStep for OcrScanner {
    fn name(&self) -> &str {
        "OcrScanner"
    }

    fn process(&self, context: &mut Context) -> Result<()> {
        if context.is_complete() {
            return Ok(());
        }

        tracing::info!("Running OCR...");
        let text = self.provider.extract_text(&context.image)?;

        // Simple heuristic extraction (very naive)
        // In a real scenario, we'd use regex or specific region OCR
        self.parse_ocr_text(text, &mut context.partial_info);

        Ok(())
    }
}

impl OcrScanner {
    fn parse_ocr_text(&self, text: String, info: &mut PartialStudentInfo) {
        // Naive parsing for example purposes
        for line in text.lines() {
            let line = line.to_lowercase();
            if line.contains("vorname:") {
                info.first_name = Some(line.replace("vorname:", "").trim().to_string());
            } else if line.contains("nachname:") {
                info.last_name = Some(line.replace("nachname:", "").trim().to_string());
            } else if line.contains("matrikelnummer:") {
                info.matriculation_number =
                    Some(line.replace("matrikelnummer:", "").trim().to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::PartialStudentInfo;
    use crate::ocr::MockOcrProvider;

    #[test]
    fn test_parse_ocr_text() {
        let text = "Vorname: Max\nNachname: Mustermann\nMatrikelnummer: 1234567".to_string();
        let scanner = OcrScanner {
            provider: Arc::new(MockOcrProvider { text }),
        };
        let mut info = PartialStudentInfo::default();

        // We need to trigger the process or call parse_ocr_text directly
        // Let's call parse_ocr_text directly to test the logic
        scanner.parse_ocr_text(
            scanner
                .provider
                .extract_text(&image::DynamicImage::new_rgb8(1, 1))
                .unwrap(),
            &mut info,
        );

        assert_eq!(info.first_name, Some("max".to_string()));
        assert_eq!(info.last_name, Some("mustermann".to_string()));
        assert_eq!(info.matriculation_number, Some("1234567".to_string()));
    }
}
