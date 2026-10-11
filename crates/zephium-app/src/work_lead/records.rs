//! Immutable extraction rows addressed by run-local keys. The model selects
//! records; only native code copies their values and media into canvas objects.
use serde::Deserialize;
use serde_json::json;
use zephium_core::work::{artifact::*, objects::*, WorkArtifactId};

#[derive(Debug)]
pub(crate) struct RecordError {
    pub message: String,
    pub reason: super::objects::ObjectRefusal,
}
impl std::fmt::Display for RecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}
impl From<&str> for RecordError {
    fn from(message: &str) -> Self {
        Self {
            message: message.into(),
            reason: super::objects::ObjectRefusal::Source,
        }
    }
}

#[derive(Default)]
pub(crate) struct Records {
    rows: Vec<Record>,
    intents: Vec<(String, zephium_core::work::collection::WorkBrowseCollection)>,
}
struct Record {
    artifact: WorkArtifactId,
    collection: Option<zephium_core::work::collection::WorkBrowseCollection>,
    row: usize,
    part: Option<zephium_core::work::WorkPartId>,
    subject: WorkSubject,
    fields: Vec<(WorkCriterion, WorkCell)>,
    evidence: Vec<WorkEvidenceLink>,
}
impl Record {
    fn field_kind(&self, name: &str) -> Option<&zephium_core::work::collection::WorkBrowseValue> {
        self.collection
            .as_ref()?
            .columns
            .iter()
            .find(|column| column.name.replace('_', " ") == name)
            .map(|column| &column.value)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Selection {
    pub source: String,
    pub annotation: Option<String>,
    #[serde(default)]
    pub recommended: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Presentation {
    pub title: String,
    pub layout: Layout,
    pub records: Vec<Selection>,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Layout {
    Picks,
    Table,
}

impl Records {
    pub fn remember_site(
        &mut self,
        host: &str,
        collection: &zephium_core::work::collection::WorkBrowseCollection,
    ) {
        let host = host.trim_start_matches("www.").to_ascii_lowercase();
        if self.intents.len() < 32
            && !self
                .intents
                .iter()
                .any(|(known, schema)| known == &host && schema.columns == collection.columns)
        {
            self.intents.push((host, collection.clone()));
        }
    }

    pub fn collection_for_site(
        &self,
        url: &str,
    ) -> Option<zephium_core::work::collection::WorkBrowseCollection> {
        let target = url::Url::parse(url).ok()?;
        let host = target.host_str()?.trim_start_matches("www.");
        let mut matching = self.intents.iter().filter(|(site, _)| site == host);
        let (_, first) = matching.next()?;
        if matching.any(|(_, schema)| schema.columns != first.columns) {
            return None;
        }
        Some(first.clone())
    }

    /// Register only already-admitted native artifacts; repeated reads of the
    /// same artifact retain their keys. Equal names never merge different rows.
    pub fn register(
        &mut self,
        artifact: &WorkArtifactV1,
        collection: Option<&zephium_core::work::collection::WorkBrowseCollection>,
    ) -> Vec<String> {
        let WorkArtifactDataV1::ComparisonMatrix {
            subjects,
            criteria,
            cells,
            ..
        } = &artifact.data
        else {
            return vec![];
        };
        if artifact.validate().is_err() {
            return vec![];
        }
        let mut keys = Vec::new();
        for (row, subject) in subjects.iter().enumerate() {
            let index = if let Some(index) = self
                .rows
                .iter()
                .position(|r| r.artifact == artifact.id && r.row == row)
            {
                index
            } else {
                // A run cannot make an unbounded second copy of its extraction.
                if self.rows.len() >= 256 {
                    break;
                }
                self.rows.push(Record {
                    artifact: artifact.id,
                    collection: collection.cloned(),
                    row,
                    part: artifact.part,
                    subject: subject.clone(),
                    fields: criteria
                        .iter()
                        .cloned()
                        .zip(cells[row].iter().cloned())
                        .collect(),
                    evidence: artifact.evidence.clone(),
                });
                self.rows.len() - 1
            };
            keys.push(format!("r{}", index + 1));
        }
        keys
    }

    pub fn collection_for_document(
        &self,
        url: &str,
    ) -> Option<zephium_core::work::collection::WorkBrowseCollection> {
        let mut candidates = self
            .rows
            .iter()
            .filter(|row| row.subject.homepage.as_deref() == Some(url));
        let first = candidates.next()?;
        // The same address on two variants is not a unique record identity.
        if candidates.any(|row| {
            row.subject.name != first.subject.name
                || row.collection.as_ref().map(|c| &c.columns)
                    != first.collection.as_ref().map(|c| &c.columns)
        }) {
            return None;
        }
        let mut collection = first.collection.clone()?;
        collection.max_items = 1;
        Some(collection)
    }

    pub fn catalog(&self, part: zephium_core::work::WorkPartId) -> String {
        let mut out = String::new();
        for (index, row) in self
            .rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row.part == Some(part))
        {
            let fields: Vec<String> = row
                .fields
                .iter()
                .filter_map(|(c, v)| {
                    display(&v.value).map(|value| format!("{}: {}", c.name, value))
                })
                .collect();
            let line = format!(
                "r{}: {} — {}\n",
                index + 1,
                row.subject.name,
                fields.join(" · ")
            );
            if out.len() + line.len() > 16 * 1024 {
                break;
            }
            out.push_str(&line);
        }
        out
    }

    pub fn present(&self, request: &Presentation) -> Result<super::objects::Proposed, RecordError> {
        if request.title.trim() != request.title
            || request.title.is_empty()
            || request.title.chars().count() > super::objects::MAX_TITLE_CHARS
            || request.title.chars().any(char::is_control)
        {
            return Err("title must be one line of 1 to 60 characters".into());
        }
        if request.records.is_empty() || request.records.len() > MAX_PICKS {
            return Err("select 1 to 12 record keys".into());
        }
        let mut selected = Vec::new();
        let mut evidence: Vec<WorkEvidenceLink> = Vec::new();
        for selection in &request.records {
            if selected
                .iter()
                .any(|(key, _, _): &(&str, &Record, u16)| *key == selection.source)
            {
                return Err("a record key may be selected only once".into());
            }
            let index = selection
                .source
                .strip_prefix('r')
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|i| *i > 0)
                .ok_or("unknown record key")?
                - 1;
            if selection.source != format!("r{}", index + 1) {
                return Err("unknown record key".into());
            }
            let record = self
                .rows
                .get(index)
                .ok_or("record key was not returned in this run")?;
            let links: Vec<&WorkEvidenceLink> = record
                .fields
                .iter()
                .flat_map(|(_, cell)| &cell.evidence)
                .filter_map(|i| record.evidence.get(usize::from(*i)))
                .collect();
            let first = links
                .first()
                .ok_or("record has no admitted field evidence")?;
            for link in &links {
                if !evidence.contains(*link) {
                    evidence.push((*link).clone());
                }
            }
            if evidence.len() > MAX_ARTIFACT_EVIDENCE {
                return Err("selection exceeds the evidence budget; select fewer records".into());
            }
            let source = evidence.iter().position(|link| link == *first).unwrap() as u16;
            selected.push((selection.source.as_str(), record, source));
        }
        let data = match request.layout {
            Layout::Picks => {
                let mut items = Vec::new();
                for ((_, record, source), selection) in selected.iter().zip(&request.records) {
                    let mut value = json!({"name":record.subject.name, "image_candidates":record.subject.image_candidates, "media_attempted":record.subject.media_attempted, "source":source, "recommended":selection.recommended});
                    for (key, text) in [
                        ("subtitle", &record.subject.descriptor),
                        ("url", &record.subject.homepage),
                        ("why", &selection.annotation),
                    ] {
                        if let Some(text) = text {
                            value[key] = json!(text);
                        }
                    }
                    if let Some(media) = &record.subject.record_media {
                        value["record_media"] = json!(media);
                    }
                    let mut facts = Vec::new();
                    let mut has_price = false;
                    for (criterion, cell) in &record.fields {
                        let Some(text) = display(&cell.value) else {
                            continue;
                        };
                        if matches!(
                            record.field_kind(&criterion.name),
                            Some(zephium_core::work::collection::WorkBrowseValue::ImageUrl)
                        ) {
                            let images = value["image_candidates"]
                                .as_array_mut()
                                .expect("native image array");
                            if !images.iter().any(|image| image.as_str() == Some(&text)) {
                                images.push(json!(text));
                            }
                            continue;
                        }
                        // Skip only values already represented exactly, never a
                        // label-based guess that silently discards another URL.
                        if record.subject.homepage.as_ref() == Some(&text)
                            || record.subject.image_candidates.contains(&text)
                        {
                            continue;
                        }
                        if !has_price
                            && super::found::price_rank(&criterion.name.to_lowercase()).is_some()
                        {
                            let mut price = json!({"display":text});
                            if let WorkCellValue::Money {
                                amount, currency, ..
                            } = &cell.value
                            {
                                price["amount"] = json!(amount);
                                price["currency"] = json!(currency);
                            }
                            value["price"] = price;
                            has_price = true;
                        } else {
                            facts.push(json!({"label":criterion.name,"value":text,"kind":"text"}));
                        }
                    }
                    value["facts"] = json!(facts);
                    items.push(
                        serde_json::from_value::<WorkPickV1>(value)
                            .map_err(|_| "native pick projection failed")?,
                    );
                }
                WorkArtifactDataV1::Picks {
                    facet: WorkPickFacetV1::Other,
                    items,
                }
            }
            Layout::Table => {
                let mut names = Vec::new();
                for (_, record, _) in &selected {
                    for (criterion, _) in &record.fields {
                        if !names.contains(&criterion.name) {
                            names.push(criterion.name.clone());
                        }
                    }
                }
                let mut columns = vec![json!({"label":"Name","kind":"text"})];
                for name in &names {
                    let link = selected
                        .iter()
                        .filter(|(_, record, _)| {
                            record
                                .fields
                                .iter()
                                .any(|(criterion, _)| criterion.name == *name)
                        })
                        .all(|(_, record, _)| {
                            matches!(
                                record.field_kind(name),
                                Some(
                                    zephium_core::work::collection::WorkBrowseValue::Url
                                        | zephium_core::work::collection::WorkBrowseValue::ImageUrl
                                )
                            )
                        });
                    columns.push(json!({"label":name,"kind":if link { "link" } else { "text" }}));
                }
                columns.push(json!({"label":"Source link","kind":"link"}));
                let annotate = request.records.iter().any(|r| r.annotation.is_some());
                if annotate {
                    columns.push(json!({"label":"Annotation","kind":"text"}));
                }
                let mut rows = Vec::new();
                for ((_, record, source), selection) in selected.iter().zip(&request.records) {
                    let mut cells = vec![record.subject.name.clone()];
                    for name in &names {
                        cells.push(
                            record
                                .fields
                                .iter()
                                .find(|(c, _)| c.name == *name)
                                .and_then(|(_, c)| display(&c.value))
                                .unwrap_or_default(),
                        );
                    }
                    cells.push(record.subject.homepage.clone().unwrap_or_default());
                    if annotate {
                        cells.push(selection.annotation.clone().unwrap_or_default());
                    }
                    let mut row = json!({"cells":cells,"source":source});
                    if record.subject.media_attempted {
                        row["entity"] = json!({"media_attempted":true});
                        if let Some(media) = &record.subject.record_media {
                            row["entity"]["record_media"] = json!(media);
                        }
                    }
                    if !record.subject.media_attempted {
                        if let Some(image) = record.subject.image_candidates.first() {
                            row["entity"] = json!({"image": image});
                        }
                    }
                    rows.push(row);
                }
                serde_json::from_value(json!({"kind":"sheet","columns":columns,"rows":rows}))
                    .map_err(|_| "native table projection failed")?
            }
        };
        if let Some(fault) = data.lead_fault(evidence.len()) {
            return Err(RecordError {
                message: format!(
                    "native record presentation: {}; original records remain unchanged",
                    fault.describe()
                ),
                reason: super::objects::ObjectRefusal::Field(fault.field),
            });
        }
        Ok(super::objects::Proposed {
            title: request.title.clone(),
            data,
            evidence,
            left_out: vec![],
        })
    }
}
fn display(value: &WorkCellValue) -> Option<String> {
    Some(match value {
        WorkCellValue::Text { text } => text.clone(),
        WorkCellValue::Measurement { value } => value.clone(),
        WorkCellValue::Money {
            amount, currency, ..
        } => format!("{amount} {currency}"),
        WorkCellValue::Rating { value } => value.to_string(),
        WorkCellValue::Presence { present } => if *present { "yes" } else { "no" }.into(),
        WorkCellValue::Unknown => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zephium_core::{ids::ProfileId, work::*};

    fn fixture() -> WorkArtifactV1 {
        let extraction = WorkArtifactId::generate();
        let execution = WorkExecutionId::generate();
        let attempt = WorkAttemptId::generate();
        let profile = ProfileId::generate();
        let work = WorkId::generate();
        let subject = |row, name: String| {
            json!({
                "name":name,"homepage":format!("https://example.com/item?variant={row}&date=2026-11-14"),
                "image_candidates":[format!("https://example.com/image?variant={row}&width=400")],
                "media_attempted":true,"record_media":{
                    "profile":profile,"work":work,"execution":execution,"attempt":attempt,"step":WorkStepId::generate(),
                    "extraction":extraction,"record":row,"source":"a".repeat(64),"digest":format!("{row:x}").repeat(64),
                    "width":128,"height":96,"origin":"native_crop"
                }
            })
        };
        serde_json::from_value(json!({
            "version":1,"id":WorkArtifactId::generate(),"execution":execution,"node":WorkPlanNodeId::generate(),"attempt":attempt,
            "output":"result","title":"Native rows","review":"source_mapped_needs_review","presentation":"automatic","general_knowledge":false,
            "evidence":[{"extraction_id":extraction,"source_id":1}],
            "data":{"kind":"comparison_matrix", "subjects":[subject(0,"Żółć東京".repeat(16)),subject(1,"Same source, different record".into())],
                "criteria":[{"name":"Price","kind":{"kind":"text"}},{"name":"Stock","kind":{"kind":"text"}}],
                "cells":[
                    [{"value":{"kind":"text","text":"129,99 zł / sztuka — cena z podatkiem"},"evidence":[0]},{"value":{"kind":"text","text":"Dostępny — wysyłka w ciągu 2 dni"},"evidence":[0]}],
                    [{"value":{"kind":"money","amount":"100.00","currency":"PLN"},"evidence":[0]},{"value":{"kind":"text","text":"Out of stock"},"evidence":[0]}]
                ]}
        })).unwrap()
    }
    fn request(layout: &str) -> Presentation {
        serde_json::from_value(json!({"title":"Results","layout":layout,"records":[{"source":"r2","annotation":"Second source row first"},{"source":"r1"}]})).unwrap()
    }
    #[test]
    fn selection_preserves_verbatim_unicode_price_stock_query_and_exact_row_media() {
        let artifact = fixture();
        artifact.validate().unwrap();
        let mut records = Records::default();
        assert_eq!(records.register(&artifact, None), ["r1", "r2"]);
        assert_eq!(records.register(&artifact, None), ["r1", "r2"]);
        let proposed = records
            .present(&request("picks"))
            .unwrap_or_else(|e| panic!("{e}"));
        let WorkArtifactDataV1::Picks { items, .. } = &proposed.data else {
            panic!()
        };
        assert_eq!(items[0].record_media.as_ref().unwrap().record, 1);
        assert_eq!(items[1].record_media.as_ref().unwrap().record, 0);
        assert_eq!(items[1].name, "Żółć東京".repeat(16));
        assert_eq!(
            items[1].price.as_ref().unwrap().display,
            "129,99 zł / sztuka — cena z podatkiem"
        );
        assert_eq!(items[1].facts[0].value, "Dostępny — wysyłka w ciągu 2 dni");
        assert_eq!(
            items[1].url.as_deref(),
            Some("https://example.com/item?variant=0&date=2026-11-14")
        );
        assert_eq!(
            items[1].image_candidates,
            ["https://example.com/image?variant=0&width=400"]
        );
        assert_eq!(
            items[0].price.as_ref().unwrap().amount.as_deref(),
            Some("100.00")
        );
        assert_eq!(items[0].why.as_deref(), Some("Second source row first"));
        assert_eq!(proposed.evidence.len(), 1);
        assert!(proposed.data.validate(proposed.evidence.len()).is_ok());
    }
    #[test]
    fn table_preserves_full_query_and_values_without_numeric_reinterpretation() {
        let mut records = Records::default();
        records.register(&fixture(), None);
        let proposed = records
            .present(&request("table"))
            .unwrap_or_else(|e| panic!("{e}"));
        let WorkArtifactDataV1::Sheet { columns, rows, .. } = &proposed.data else {
            panic!()
        };
        assert_eq!(
            rows[0]
                .entity
                .as_ref()
                .unwrap()
                .record_media
                .as_ref()
                .unwrap()
                .record,
            1
        );
        assert_eq!(
            rows[1]
                .entity
                .as_ref()
                .unwrap()
                .record_media
                .as_ref()
                .unwrap()
                .record,
            0
        );
        let mut untrusted = proposed.data.clone();
        untrusted.clear_record_media();
        let WorkArtifactDataV1::Sheet { rows: cleared, .. } = untrusted else {
            panic!()
        };
        assert!(cleared
            .iter()
            .all(|row| row.entity.as_ref().unwrap().record_media.is_none()));
        let link = columns
            .iter()
            .position(|c| c.label == "Source link")
            .unwrap();
        assert_eq!(
            rows[1].cells[link],
            "https://example.com/item?variant=0&date=2026-11-14"
        );
        assert_eq!(rows[1].cells[0], "Żółć東京".repeat(16));
        assert_eq!(rows[1].cells[1], "129,99 zł / sztuka — cena z podatkiem");
        assert_eq!(rows[1].cells[2], "Dostępny — wysyłka w ciągu 2 dni");
    }
    #[test]
    fn exact_record_detail_read_keeps_requested_stock_columns_but_not_ambiguous_variants() {
        let artifact = fixture();
        let collection: zephium_core::work::collection::WorkBrowseCollection = serde_json::from_value(json!({
            "title":"Offers", "max_items":5,"columns":[
                {"name":"price","value":{"kind":"text"},"required":false,"extraction":"verbatim"},
                {"name":"stock","value":{"kind":"text"},"required":false,"extraction":"verbatim"}
            ]
        })).unwrap();
        let mut bank = Records::default();
        bank.register(&artifact, Some(&collection));
        let url = "https://example.com/item?variant=0&date=2026-11-14";
        let retained = bank.collection_for_document(url).unwrap();
        assert_eq!(retained.max_items, 1);
        assert_eq!(retained.columns, collection.columns);
        assert!(bank
            .collection_for_document("https://example.com/item")
            .is_none());
        bank.rows[1].subject.homepage = Some(url.into());
        assert!(bank.collection_for_document(url).is_none());
    }

    #[test]
    fn a_withheld_image_url_is_not_misclassified_as_an_oversized_text_fact() {
        let mut artifact = fixture();
        let image = format!(
            "https://images.example.com/{}.jpg?width=400",
            "a".repeat(180)
        );
        let WorkArtifactDataV1::ComparisonMatrix {
            subjects,
            criteria,
            cells,
            ..
        } = &mut artifact.data
        else {
            panic!()
        };
        for subject in subjects {
            subject.image_candidates.clear();
            subject.record_media = None;
        }
        criteria.push(WorkCriterion {
            name: "image".into(),
            kind: WorkCriterionKind::Text,
        });
        for row in cells {
            row.push(WorkCell {
                value: WorkCellValue::Text {
                    text: image.clone(),
                },
                evidence: vec![0],
                note: None,
                general_knowledge: false,
            });
        }
        let schema=serde_json::from_value(json!({"title":"Items","max_items":3,"columns":[{"name":"image","value":{"kind":"image_url"},"required":false}]})).unwrap();
        let mut bank = Records::default();
        bank.register(&artifact, Some(&schema));
        let picks = bank
            .present(&request("picks"))
            .unwrap_or_else(|e| panic!("{e}"));
        let WorkArtifactDataV1::Picks { items, .. } = picks.data else {
            panic!()
        };
        assert_eq!(items[0].image_candidates, [image.clone()]);
        assert!(
            items[0].media_attempted && items[0].record_media.is_none(),
            "a withheld native image must stay withheld"
        );
        assert!(items[0].facts.iter().all(|fact| fact.label != "image"));
        let table = bank
            .present(&request("table"))
            .unwrap_or_else(|e| panic!("{e}"));
        let WorkArtifactDataV1::Sheet { columns, rows, .. } = table.data else {
            panic!()
        };
        let index = columns
            .iter()
            .position(|column| column.label == "image")
            .unwrap();
        assert_eq!(columns[index].kind, WorkSheetColumnKindV1::Link);
        assert_eq!(rows[0].cells[index], image);
    }

    #[test]
    fn failed_catalog_read_cannot_erase_requested_columns_from_same_site_fallback() {
        let schema: zephium_core::work::collection::WorkBrowseCollection = serde_json::from_value(json!({
            "title":"Books", "max_items":3,"columns":[
                {"name":"price","value":{"kind":"text"},"required":false,"extraction":"verbatim"},
                {"name":"availability","value":{"kind":"text"},"required":false,"extraction":"verbatim"}
            ]
        })).unwrap();
        let mut bank = Records::default();
        bank.remember_site("shop.example.com", &schema);
        assert!(bank.rows.is_empty());
        let retained = bank
            .collection_for_site("https://www.shop.example.com/book?id=3")
            .unwrap();
        assert_eq!(retained.columns, schema.columns);
        assert!(bank
            .collection_for_site("https://other.example.com/book?id=3")
            .is_none());
        assert!(bank
            .collection_for_site("https://shop.example.com.attacker.test/book")
            .is_none());
        let mut conflicting = schema.clone();
        conflicting.columns[1].name = "publication_date".into();
        bank.remember_site("shop.example.com", &conflicting);
        assert!(bank
            .collection_for_site("https://shop.example.com/book?id=3")
            .is_none());
    }

    #[test]
    fn keys_cannot_invent_or_rewrite_records_and_overflow_is_never_truncated() {
        let mut records = Records::default();
        records.register(&fixture(), None);
        let mut selection = request("picks");
        selection.records[0].source = "r0".into();
        assert!(records.present(&selection).is_err());
        selection.records[0].source = "r1".into();
        assert!(records.present(&selection).is_err());
        selection.records[0].source = "r3".into();
        assert!(records.present(&selection).is_err());
        selection.records[0].source = "r01".into();
        assert!(records.present(&selection).is_err());
        assert!(
            serde_json::from_value::<Selection>(json!({"source":"r1","price":"free"})).is_err()
        );
        records.rows[0].subject.name = "界".repeat(200);
        assert!(records.present(&request("picks")).is_ok());
        records.rows[0].subject.name = "界".repeat(201);
        let error = records.present(&request("picks")).err().unwrap();
        assert!(matches!(
            error.reason,
            super::super::objects::ObjectRefusal::Field(WorkArtifactField::PickName)
        ));
        assert_eq!(records.rows[0].subject.name.chars().count(), 201);
    }
}
