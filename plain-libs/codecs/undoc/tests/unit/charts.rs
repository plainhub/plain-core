use super::*;

#[test]
fn test_parse_bar_chart() {
    let xml = r#"<?xml version="1.0"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <c:chart>
    <c:plotArea>
      <c:barChart>
        <c:ser>
          <c:tx>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>2010</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:tx>
          <c:cat>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>Q1</c:v></c:pt>
                <c:pt idx="1"><c:v>Q2</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:cat>
          <c:val>
            <c:numRef>
              <c:numCache>
                <c:pt idx="0"><c:v>100</c:v></c:pt>
                <c:pt idx="1"><c:v>150</c:v></c:pt>
              </c:numCache>
            </c:numRef>
          </c:val>
        </c:ser>
        <c:ser>
          <c:tx>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>2011</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:tx>
          <c:cat>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>Q1</c:v></c:pt>
                <c:pt idx="1"><c:v>Q2</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:cat>
          <c:val>
            <c:numRef>
              <c:numCache>
                <c:pt idx="0"><c:v>120</c:v></c:pt>
                <c:pt idx="1"><c:v>180</c:v></c:pt>
              </c:numCache>
            </c:numRef>
          </c:val>
        </c:ser>
      </c:barChart>
    </c:plotArea>
  </c:chart>
</c:chartSpace>"#;

    let chart_data = parse_chart_xml(xml).unwrap();

    assert_eq!(chart_data.categories, vec!["Q1", "Q2"]);
    assert_eq!(chart_data.series.len(), 2);
    assert_eq!(chart_data.series[0].name, "2010");
    assert_eq!(chart_data.series[0].values, vec![Some(100.0), Some(150.0)]);
    assert_eq!(chart_data.series[1].name, "2011");
    assert_eq!(chart_data.series[1].values, vec![Some(120.0), Some(180.0)]);
}

#[test]
fn test_chart_to_table() {
    let chart_data = ChartData {
        title: Some("Revenue".to_string()),
        categories: vec!["Q1".to_string(), "Q2".to_string()],
        series: vec![
            ChartSeries {
                name: "2010".to_string(),
                values: vec![Some(100.0), Some(150.0)],
            },
            ChartSeries {
                name: "2011".to_string(),
                values: vec![Some(120.0), Some(180.0)],
            },
        ],
    };

    let table = chart_data.to_table();

    assert_eq!(table.row_count(), 3); // header + 2 data rows
    assert_eq!(table.column_count(), 3); // Category + 2 series

    // Check header
    assert_eq!(table.rows[0].cells[0].plain_text(), "Category");
    assert_eq!(table.rows[0].cells[1].plain_text(), "2010");
    assert_eq!(table.rows[0].cells[2].plain_text(), "2011");

    // Check data
    assert_eq!(table.rows[1].cells[0].plain_text(), "Q1");
    assert_eq!(table.rows[1].cells[1].plain_text(), "100");
    assert_eq!(table.rows[2].cells[0].plain_text(), "Q2");
}

#[test]
fn test_parse_chart_title_from_rich_text() {
    let xml = r#"<?xml version="1.0"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"
              xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <c:chart>
    <c:title>
      <c:tx>
        <c:rich>
          <a:p>
            <a:r><a:t>Revenue</a:t></a:r>
            <a:r><a:t> Growth</a:t></a:r>
          </a:p>
        </c:rich>
      </c:tx>
    </c:title>
    <c:plotArea>
      <c:barChart>
        <c:ser>
          <c:tx>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>2024</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:tx>
          <c:cat>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>Q1</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:cat>
          <c:val>
            <c:numRef>
              <c:numCache>
                <c:pt idx="0"><c:v>42</c:v></c:pt>
              </c:numCache>
            </c:numRef>
          </c:val>
        </c:ser>
      </c:barChart>
    </c:plotArea>
  </c:chart>
</c:chartSpace>"#;

    let chart_data = parse_chart_xml(xml).unwrap();

    assert_eq!(chart_data.title.as_deref(), Some("Revenue Growth"));
}

#[test]
fn test_chart_to_table_keeps_missing_values_blank() {
    let xml = r#"<?xml version="1.0"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <c:chart>
    <c:plotArea>
      <c:lineChart>
        <c:ser>
          <c:tx>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>Series A</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:tx>
          <c:cat>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>Q1</c:v></c:pt>
                <c:pt idx="1"><c:v>Q2</c:v></c:pt>
                <c:pt idx="2"><c:v>Q3</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:cat>
          <c:val>
            <c:numRef>
              <c:numCache>
                <c:pt idx="0"><c:v>100</c:v></c:pt>
                <c:pt idx="2"><c:v>150</c:v></c:pt>
              </c:numCache>
            </c:numRef>
          </c:val>
        </c:ser>
      </c:lineChart>
    </c:plotArea>
  </c:chart>
</c:chartSpace>"#;

    let chart_data = parse_chart_xml(xml).unwrap();
    let table = chart_data.to_table();

    assert_eq!(table.rows[2].cells[0].plain_text(), "Q2");
    assert_eq!(table.rows[2].cells[1].plain_text(), "");
}

#[test]
fn test_parse_chart_title_preserves_raw_malformed_entity() {
    let xml = r#"<?xml version="1.0"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"
              xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
  <c:chart>
    <c:title>
      <c:tx>
        <c:rich>
          <a:p>
            <a:r><a:t>Bad &bogus; title</a:t></a:r>
          </a:p>
        </c:rich>
      </c:tx>
    </c:title>
    <c:plotArea>
      <c:barChart>
        <c:ser>
          <c:tx>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>S</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:tx>
          <c:cat>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>Q1</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:cat>
          <c:val>
            <c:numRef>
              <c:numCache>
                <c:pt idx="0"><c:v>1</c:v></c:pt>
              </c:numCache>
            </c:numRef>
          </c:val>
        </c:ser>
      </c:barChart>
    </c:plotArea>
  </c:chart>
</c:chartSpace>"#;

    let chart_data = parse_chart_xml(xml).unwrap();
    assert_eq!(chart_data.title.as_deref(), Some("Bad &bogus; title"));
}

#[test]
fn test_parse_chart_invalid_numeric_value_errors() {
    let xml = r#"<?xml version="1.0"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <c:chart>
    <c:plotArea>
      <c:lineChart>
        <c:ser>
          <c:tx>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>Series A</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:tx>
          <c:cat>
            <c:strRef>
              <c:strCache>
                <c:pt idx="0"><c:v>Q1</c:v></c:pt>
              </c:strCache>
            </c:strRef>
          </c:cat>
          <c:val>
            <c:numRef>
              <c:numCache>
                <c:pt idx="0"><c:v>not-a-number</c:v></c:pt>
              </c:numCache>
            </c:numRef>
          </c:val>
        </c:ser>
      </c:lineChart>
    </c:plotArea>
  </c:chart>
</c:chartSpace>"#;

    let err = parse_chart_xml(xml).unwrap_err();
    assert!(matches!(
        err,
        Error::InvalidData(message) if message == "invalid chart numeric value: not-a-number"
    ));
}

#[test]
fn test_format_number() {
    assert_eq!(format_number(100.0), "100");
    assert_eq!(format_number(8.3), "8.3");
    assert_eq!(format_number(8.300000), "8.3");
    assert_eq!(format_number(12.345678), "12.345678");
    assert_eq!(format_number(12.3456789), "12.3456789");
}

#[test]
fn test_chart_text_mixed_entities_preserve_legitimate_and_malformed() {
    let chart_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart">
  <c:chart><c:title><c:tx><c:rich>
    <a:p xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
      <a:r><a:t>A &amp; B &bogus; C</a:t></a:r>
    </a:p>
  </c:rich></c:tx></c:title></c:chart>
</c:chartSpace>"#;

    let data = parse_chart_xml(chart_xml).expect("chart parses");
    let title = data.title.expect("title extracted");
    assert!(
        title.contains("A & B &bogus; C"),
        "legitimate decoded + malformed preserved; got {title:?}"
    );
    assert!(
        !title.contains("A &amp; B"),
        "legitimate entity must not remain escaped; got {title:?}"
    );
}
