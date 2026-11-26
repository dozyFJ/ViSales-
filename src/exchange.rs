use crate::store::{Product, Sale, parse_money};
use anyhow::{Context, Result, ensure};
use calamine::{Reader, open_workbook_auto};
use rust_xlsxwriter::Workbook;
use std::{collections::HashSet, path::Path};

const COLUMNS: [&str; 5] = [
    "Штрихкод",
    "Назва",
    "Собівартість, грн",
    "Ціна, грн",
    "Залишок",
];

pub fn import(path: &Path) -> Result<Vec<Product>> {
    let mut workbook = open_workbook_auto(path)?;
    let sheet = workbook
        .worksheet_range_at(0)
        .context("Книга не має аркушів")??;
    let mut rows = sheet.rows();
    let header = rows.next().context("Аркуш порожній")?;
    ensure!(
        header
            .iter()
            .take(4)
            .map(ToString::to_string)
            .eq(COLUMNS[..4].iter().copied()),
        "Скористайтеся заголовками з експорту каталогу"
    );
    let mut seen = HashSet::new();
    let mut products = Vec::new();
    for (index, row) in rows.enumerate() {
        if row.iter().all(|cell| cell.to_string().trim().is_empty()) {
            continue;
        }
        let cell = |column: usize| row.get(column).map(ToString::to_string).unwrap_or_default();
        let barcode = cell(0).trim().to_owned();
        ensure!(
            seen.insert(barcode.clone()),
            "Рядок {}: повторний штрихкод",
            index + 2
        );
        products.push(Product {
            id: 0,
            barcode,
            name: cell(1),
            category: None,
            cost: parse_money(&cell(2))
                .with_context(|| format!("Рядок {}: собівартість", index + 2))?,
            price: parse_money(&cell(3)).with_context(|| format!("Рядок {}: ціна", index + 2))?,
            stock: 0,
        });
    }
    Ok(products)
}

pub fn export(path: &Path, products: &[Product], sales: &[Sale]) -> Result<()> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet().set_name("Каталог")?;
    sheet
        .set_column_width(0, 21)?
        .set_column_width(1, 32)?
        .set_column_width(2, 22)?;
    for (col, label) in COLUMNS.iter().enumerate() {
        sheet.write_string(0, col as u16, *label)?;
    }
    for (index, p) in products.iter().enumerate() {
        let row = (index + 1) as u32;
        sheet
            .write_string(row, 0, &p.barcode)?
            .write_string(row, 1, &p.name)?;
        sheet
            .write_number(row, 2, p.cost as f64 / 100.)?
            .write_number(row, 3, p.price as f64 / 100.)?
            .write_number(row, 4, p.stock as f64)?;
    }
    let sheet = workbook.add_worksheet().set_name("Продажі")?;
    sheet.set_column_width(1, 24)?.set_column_width(4, 24)?;
    for (col, label) in [
        "Номер",
        "Дата UTC",
        "Сума, грн",
        "Валовий прибуток, грн",
        "Повернено UTC",
    ]
    .iter()
    .enumerate()
    {
        sheet.write_string(0, col as u16, *label)?;
    }
    for (index, s) in sales.iter().enumerate() {
        let row = (index + 1) as u32;
        sheet
            .write_number(row, 0, s.id as f64)?
            .write_string(row, 1, &s.created)?;
        sheet
            .write_number(row, 2, s.total as f64 / 100.)?
            .write_number(row, 3, s.margin as f64 / 100.)?;
        if let Some(date) = &s.returned {
            sheet.write_string(row, 4, date)?;
        }
    }
    workbook.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xlsx_preserves_barcode_and_prices() -> Result<()> {
        let directory = std::env::temp_dir().join(format!("visales-xlsx-{}", std::process::id()));
        std::fs::create_dir_all(&directory)?;
        let path = directory.join("round-trip.xlsx");
        let product = Product {
            id: 1,
            barcode: "000123".into(),
            name: "Кава".into(),
            category: None,
            cost: 110,
            price: 999,
            stock: 7,
        };
        let result = (|| {
            export(&path, &[product.clone()], &[])?;
            let imported = import(&path)?;
            assert_eq!(imported.len(), 1);
            assert_eq!(imported[0].barcode, product.barcode);
            assert_eq!(imported[0].name, product.name);
            assert_eq!((imported[0].cost, imported[0].price), (110, 999));
            assert_eq!(imported[0].stock, 0);
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(directory);
        result
    }
}
