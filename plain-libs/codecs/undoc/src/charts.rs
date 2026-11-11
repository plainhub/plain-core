//! PPTX chart parsing module.
//!
//! Extracts data from chart XML files and converts them to tables for RAG-ready output.

use crate::error::{Error, Result};
use crate::model::{Cell, Row, Table};

/// Parsed chart data
#[derive(Debug, Clone)]
pub struct ChartData {
    /// Chart title (if available)
    pub title: Option<String>,
    /// Category labels (X-axis)
    pub categories: Vec<String>,
    /// Series data
    pub series: Vec<ChartSeries>,
}

/// A data series in a chart
#[derive(Debug, Clone)]
pub struct ChartSeries {
    /// Series name (legend label)
    pub name: String,
    /// Data values
    pub values: Vec<Option<f64>>,
}

impl ChartData {
    /// Convert chart data to a Table for markdown rendering
    pub fn to_table(&self) -> Table {
        let mut table = Table::new();

        // Build header row: Category | Series1 | Series2 | ...
        let mut header_cells = vec![Cell::header("Category")];
        for series in &self.series {
            header_cells.push(Cell::header(&series.name));
        }
        let mut header = Row::header(header_cells);
        header.is_header = true;
        table.add_row(header);

        // Build data rows
        for (i, category) in self.categories.iter().enumerate() {
            let mut cells = vec![Cell::with_text(category)];
            for series in &self.series {
                let formatted = series
                    .values
                    .get(i)
                    .and_then(|value| *value)
                    .map(format_number)
                    .unwrap_or_default();
                cells.push(Cell::with_text(&formatted));
            }
            table.add_row(Row {
                cells,
                is_header: false,
                height: None,
            });
        }

        table
    }

    /// Check if chart data is empty
    pub fn is_empty(&self) -> bool {
        self.categories.is_empty() || self.series.is_empty()
    }
}

/// Format a number, removing unnecessary trailing zeros
fn format_number(n: f64) -> String {
    n.to_string()
}

/// Parse chart XML to extract data
pub fn parse_chart_xml(xml: &str) -> Result<ChartData> {
    let mut reader = crate::decode::reader_for(xml);
    reader.config_mut().trim_text(false);

    let mut chart_data = ChartData {
        title: None,
        categories: Vec::new(),
        series: Vec::new(),
    };

    let mut buf = Vec::new();

    // State tracking
    let mut in_title = false;
    let mut in_ser = false;
    let mut in_tx = false; // Series name / title text
    let mut in_cat = false; // Categories
    let mut in_val = false; // Values
    let mut in_pt = false;
    let mut in_text_node = false;

    let mut current_series_name = String::new();
    let mut current_values: Vec<Option<f64>> = Vec::new();
    let mut current_title = String::new();
    let mut current_text = String::new();
    let mut current_point_text = String::new();
    let mut pt_idx: Option<usize> = None;

    // Temporary storage for categories (only from first series)
    let mut temp_categories: Vec<String> = Vec::new();
    let mut categories_captured = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(plain_libs::xml::events::Event::Start(ref e)) => {
                let local_name = e.name().local_name();
                match local_name.as_ref() {
                    "title" => {
                        in_title = true;
                        current_title.clear();
                    }
                    "ser" => {
                        in_ser = true;
                        current_series_name.clear();
                        current_values.clear();
                    }
                    "tx" if in_ser || in_title => {
                        in_tx = true;
                    }
                    "cat" if in_ser => {
                        in_cat = true;
                    }
                    "val" if in_ser => {
                        in_val = true;
                    }
                    "pt" => {
                        in_pt = true;
                        current_point_text.clear();
                        // Get idx attribute
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == "idx" {
                                if let Ok(idx) = attr.value.parse::<usize>() {
                                    pt_idx = Some(idx);
                                }
                            }
                        }
                    }
                    "v" | "t" => {
                        in_text_node = true;
                        current_text.clear();
                    }
                    _ => {}
                }
            }
            Ok(plain_libs::xml::events::Event::End(ref e)) => {
                let local_name = e.name().local_name();
                match local_name.as_ref() {
                    "title" => {
                        let title = current_title.trim();
                        if !title.is_empty() {
                            chart_data.title = Some(title.to_string());
                        }
                        in_title = false;
                    }
                    "ser" => {
                        // Save series if we have data
                        if !current_series_name.is_empty() || !current_values.is_empty() {
                            let name = if current_series_name.is_empty() {
                                format!("Series {}", chart_data.series.len() + 1)
                            } else {
                                current_series_name.clone()
                            };
                            chart_data.series.push(ChartSeries {
                                name,
                                values: current_values.clone(),
                            });
                        }

                        // Capture categories from first series
                        if !categories_captured && !temp_categories.is_empty() {
                            chart_data.categories = temp_categories.clone();
                            categories_captured = true;
                        }
                        temp_categories.clear();

                        in_ser = false;
                    }
                    "tx" => {
                        in_tx = false;
                    }
                    "cat" => {
                        in_cat = false;
                    }
                    "val" => {
                        in_val = false;
                    }
                    "pt" => {
                        let point_text = current_point_text.trim();

                        if in_title && !point_text.is_empty() {
                            current_title.push_str(point_text);
                        } else if in_ser && in_tx && !point_text.is_empty() {
                            current_series_name.push_str(point_text);
                        } else if in_cat && !point_text.is_empty() {
                            temp_categories.push(point_text.to_string());
                        } else if in_val && !point_text.is_empty() {
                            let val = point_text.parse::<f64>().map_err(|_| {
                                Error::InvalidData(format!(
                                    "invalid chart numeric value: {point_text}"
                                ))
                            })?;
                            if let Some(idx) = pt_idx {
                                while current_values.len() <= idx {
                                    current_values.push(None);
                                }
                                current_values[idx] = Some(val);
                            } else {
                                current_values.push(Some(val));
                            }
                        }

                        in_pt = false;
                        pt_idx = None;
                        current_point_text.clear();
                    }
                    "v" | "t" => {
                        if in_text_node {
                            if in_pt {
                                current_point_text.push_str(&current_text);
                            } else if in_title {
                                current_title.push_str(&current_text);
                            } else if in_ser && in_tx {
                                current_series_name.push_str(&current_text);
                            }
                        }
                        in_text_node = false;
                    }
                    _ => {}
                }
            }
            Ok(plain_libs::xml::events::Event::Text(ref e)) if in_text_node => {
                current_text.push_str(&crate::decode::decode_text_lossy(e));
            }
            // quick-xml 0.40+ delivers entity refs as separate events; mirror the
            // Text arm so a series name like "R&amp;D" keeps its ampersand.
            Ok(plain_libs::xml::events::Event::GeneralRef(ref e)) if in_text_node => {
                current_text.push_str(&crate::decode::resolve_general_ref(e));
            }
            Ok(plain_libs::xml::events::Event::Eof) => break,
            Err(e) => return Err(e.into()),
            _ => {}
        }
        buf.clear();
    }

    Ok(chart_data)
}

#[cfg(test)]
#[path = "../tests/unit/charts.rs"]
mod tests;
