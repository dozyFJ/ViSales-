use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, TransactionBehavior::Immediate, params};
use std::{collections::BTreeMap, path::Path, time::Duration};

pub struct Store(Connection);

#[derive(Clone)]
pub struct Product {
    pub id: i64,
    pub barcode: String,
    pub name: String,
    pub cost: i64,
    pub price: i64,
    pub stock: i64,
}

pub struct Sale {
    pub id: i64,
    pub created: String,
    pub returned: Option<String>,
    pub total: i64,
    pub margin: i64,
}

pub fn money(value: i64) -> String {
    let magnitude = value.unsigned_abs();
    let sign = if value < 0 { "−" } else { "" };
    format!("{sign}{},{:02} ₴", magnitude / 100, magnitude % 100)
}

pub fn parse_money(value: &str) -> Result<i64> {
    let value = value.trim();
    let (whole, fraction) = value.split_once([',', '.']).unwrap_or((value, ""));
    ensure!(
        !whole.is_empty()
            && whole.bytes().all(|b| b.is_ascii_digit())
            && fraction.len() <= 2
            && fraction.bytes().all(|b| b.is_ascii_digit()),
        "Вкажіть суму у гривнях, наприклад 49,90"
    );
    let hryvnias: i64 = whole.parse().context("Некоректна сума")?;
    ensure!(hryvnias <= 1_000_000, "Сума перевищує допустиме значення");
    let kopiykas = fraction.parse::<i64>().unwrap_or_default();
    let total = hryvnias * 100
        + if fraction.len() == 1 {
            kopiykas * 10
        } else {
            kopiykas
        };
    ensure!(total <= 100_000_000, "Сума перевищує допустиме значення");
    Ok(total)
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Self(conn))
    }

    pub fn products(&self) -> Result<Vec<Product>> {
        let mut query = self
            .0
            .prepare("SELECT id,barcode,name,cost,price,stock FROM product ORDER BY name,id")?;
        Ok(query
            .query_map([], |r| {
                Ok(Product {
                    id: r.get(0)?,
                    barcode: r.get(1)?,
                    name: r.get(2)?,
                    cost: r.get(3)?,
                    price: r.get(4)?,
                    stock: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn save_products(&mut self, products: &[Product]) -> Result<()> {
        let tx = self.0.transaction_with_behavior(Immediate)?;
        for p in products {
            ensure!(
                !p.barcode.trim().is_empty() && !p.name.trim().is_empty(),
                "Вкажіть назву та штрихкод"
            );
            tx.execute(
                "INSERT INTO product(barcode,name,cost,price) VALUES(?1,?2,?3,?4)
                 ON CONFLICT(barcode) DO UPDATE SET name=excluded.name,cost=excluded.cost,price=excluded.price",
                params![p.barcode.trim(), p.name.trim(), p.cost, p.price],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn add_demo_products(&mut self) -> Result<usize> {
        // These are internal barcodes and fixed seed values in kopiykas.
        let items = [
            ("Цемент М500, 25 кг", 14800, 19900, 32),
            ("Шпаклівка фінішна, 5 кг", 10500, 14900, 18),
            ("Піна монтажна, 750 мл", 15100, 21900, 24),
            ("Саморізи по дереву 4×40, 100 шт.", 3100, 4900, 50),
            ("Рулетка будівельна, 5 м", 8100, 12900, 14),
            ("Рукавички робочі, пара", 1700, 2900, 48),
            ("Лампа LED E27, 9 Вт", 3500, 5900, 36),
            ("Подовжувач на 3 розетки, 3 м", 16600, 23900, 9),
            ("Батарейки AA, 4 шт.", 5400, 7900, 26),
            ("Рис довгозернистий, 1 кг", 4750, 6290, 27),
            ("Кава мелена, 250 г", 12400, 16900, 16),
            ("Чай чорний, 25 пакетиків", 3900, 5950, 23),
            ("Вода негазована, 1,5 л", 1300, 2190, 42),
            ("Засіб для миття посуду, 500 мл", 3600, 5490, 20),
            ("Серветки паперові, 100 шт.", 1900, 3290, 28),
            ("Губки кухонні, 5 шт.", 2100, 3490, 25),
            ("Зошит у клітинку, 48 аркушів", 1500, 2490, 40),
            ("Ручка кулькова синя", 550, 1290, 65),
            ("Ґрунт універсальний, 5 л", 3800, 5990, 15),
            ("Корм для котів сухий, 400 г", 6300, 8990, 22),
            ("Корм для собак вологий, 100 г", 1290, 2190, 44),
            ("Наповнювач для котів, 5 л", 9200, 13900, 12),
            ("Поводок для собак, 1,5 м", 7900, 11900, 8),
            ("Іграшка для котів, м'ячик", 1600, 2990, 20),
        ];
        let tx = self.0.transaction_with_behavior(Immediate)?;
        let mut added = 0;
        {
            let mut insert = tx.prepare(
                "INSERT INTO product(barcode,name,cost,price,stock) VALUES(?1,?2,?3,?4,?5)
                 ON CONFLICT(barcode) DO NOTHING",
            )?;
            for (index, (name, cost, price, stock)) in items.into_iter().enumerate() {
                let digits = format!("20000000{:04}", index + 1);
                let sum: u8 = digits
                    .bytes()
                    .enumerate()
                    .map(|(position, digit)| (digit - b'0') * if position % 2 == 0 { 1 } else { 3 })
                    .sum();
                let barcode = format!("{digits}{}", (10 - sum % 10) % 10);
                added += insert.execute(params![
                    barcode,
                    format!("{name} [Тест]"),
                    cost,
                    price,
                    stock
                ])?;
            }
        }
        tx.commit()?;
        Ok(added)
    }

    pub fn count(&mut self, product: i64, counted: i64) -> Result<()> {
        let tx = self.0.transaction_with_behavior(Immediate)?;
        ensure!(tx.execute(
            "INSERT INTO stock_count(product,previous,counted) SELECT id,stock,?2 FROM product WHERE id=?1",
            params![product,counted],
        )? == 1, "Виберіть товар");
        tx.execute(
            "UPDATE product SET stock=?2 WHERE id=?1",
            params![product, counted],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn stock_history(&self) -> Result<Vec<(String, String, i64, i64)>> {
        let mut q = self.0.prepare(
            "SELECT stock_count.created, product.name, previous, counted
             FROM stock_count JOIN product ON product.id=stock_count.product ORDER BY stock_count.id DESC"
        )?;
        Ok(
            q.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<rusqlite::Result<_>>()?,
        )
    }

    pub fn supplier(&self, name: &str) -> Result<()> {
        ensure!(!name.trim().is_empty(), "Вкажіть назву постачальника");
        self.0.execute(
            "INSERT INTO supplier(name) VALUES(?1) ON CONFLICT(name) DO NOTHING",
            [name.trim()],
        )?;
        Ok(())
    }

    pub fn suppliers(&self) -> Result<Vec<(i64, String)>> {
        let mut q = self
            .0
            .prepare("SELECT id,name FROM supplier ORDER BY name")?;
        Ok(q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn receive(
        &mut self,
        supplier: i64,
        reference: &str,
        product: i64,
        quantity: i64,
        cost: i64,
    ) -> Result<()> {
        ensure!(
            !reference.trim().is_empty() && quantity > 0,
            "Вкажіть номер накладної та кількість"
        );
        let tx = self.0.transaction_with_behavior(Immediate)?;
        tx.execute("INSERT INTO delivery(supplier,reference,product,quantity,unit_cost) VALUES(?1,?2,?3,?4,?5)",
            params![supplier,reference.trim(),product,quantity,cost])?;
        tx.execute(
            "UPDATE product SET cost=(stock*cost+?2*?3+(stock+?2)/2)/(stock+?2),stock=stock+?2 WHERE id=?1",
            params![product,quantity,cost],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn deliveries(&self) -> Result<Vec<(String, String, String, i64, i64)>> {
        let mut q = self.0.prepare(
            "SELECT reference,supplier.name,product.name,quantity,unit_cost
             FROM delivery JOIN supplier ON supplier.id=delivery.supplier
             JOIN product ON product.id=delivery.product ORDER BY delivery.id DESC",
        )?;
        Ok(q.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?)
    }

    pub fn sell(&mut self, cart: &BTreeMap<i64, (i64, i64)>) -> Result<i64> {
        ensure!(!cart.is_empty(), "Кошик порожній");
        let tx = self.0.transaction_with_behavior(Immediate)?;
        tx.execute("INSERT INTO sale DEFAULT VALUES", [])?;
        let id = tx.last_insert_rowid();
        for (&product, &(quantity, expected_price)) in cart {
            ensure!(quantity > 0, "Некоректна кількість");
            let changed = tx.execute(
                "UPDATE product SET stock=stock-?2 WHERE id=?1 AND stock>=?2 AND price=?3",
                params![product, quantity, expected_price],
            )?;
            ensure!(changed == 1, "Ціна або залишок змінилися. Перевірте кошик.");
            tx.execute(
                "INSERT INTO sale_line SELECT ?1,id,name,?3,price,cost FROM product WHERE id=?2",
                params![id, product, quantity],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn refund(&mut self, id: i64) -> Result<()> {
        let tx = self.0.transaction_with_behavior(Immediate)?;
        ensure!(
            tx.execute(
                "UPDATE sale SET returned_at=CURRENT_TIMESTAMP WHERE id=?1 AND returned_at IS NULL",
                [id],
            )? == 1,
            "Продаж уже повернено або відсутній"
        );
        tx.execute(
            "UPDATE product SET
                 cost=(stock*cost+line.quantity*line.unit_cost+(stock+line.quantity)/2)/(stock+line.quantity),
                 stock=stock+line.quantity
             FROM sale_line AS line WHERE line.sale=?1 AND line.product=product.id",
            [id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn sales(&self) -> Result<Vec<Sale>> {
        let mut q = self.0.prepare(
            "SELECT sale.id,created,returned_at,SUM(quantity*unit_price),SUM(quantity*(unit_price-unit_cost))
             FROM sale JOIN sale_line ON sale.id=sale_line.sale GROUP BY sale.id ORDER BY sale.id DESC"
        )?;
        Ok(q.query_map([], |r| {
            Ok(Sale {
                id: r.get(0)?,
                created: r.get(1)?,
                returned: r.get(2)?,
                total: r.get(3)?,
                margin: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_catalog_can_be_added_again_without_resetting_stock() -> Result<()> {
        let mut store = Store::open(Path::new(":memory:"))?;
        assert_eq!(store.add_demo_products()?, 24);
        let product = store.products()?.remove(0);
        assert!(product.stock > 0);
        store.count(product.id, 1)?;
        assert_eq!(store.add_demo_products()?, 0);
        assert_eq!(store.products()?.remove(0).stock, 1);
        Ok(())
    }

    #[test]
    fn cash_sale_is_atomic_and_refund_materializes_once() -> Result<()> {
        let mut store = Store::open(Path::new(":memory:"))?;
        store.save_products(&[
            Product {
                id: 0,
                barcode: "001".into(),
                name: "Кава".into(),
                cost: 4500,
                price: 7200,
                stock: 0,
            },
            Product {
                id: 0,
                barcode: "002".into(),
                name: "Чай".into(),
                cost: 2000,
                price: 3300,
                stock: 0,
            },
        ])?;
        let products = store.products()?;
        let coffee = &products[0];
        let tea = &products[1];
        store.supplier("Ростер")?;
        let supplier = store.suppliers()?[0].0;
        store.receive(supplier, "Н-01", coffee.id, 4, 4500)?;
        assert!(store.receive(supplier, "Н-01", coffee.id, 4, 4500).is_err());
        let invalid = BTreeMap::from([(coffee.id, (2, 7200)), (tea.id, (1, 3300))]);
        assert!(store.sell(&invalid).is_err());
        assert!(store.sales()?.is_empty());
        assert_eq!(store.products()?[0].stock, 4);

        store.count(tea.id, 1)?;
        let id = store.sell(&invalid)?;
        let sales = store.sales()?;
        assert_eq!((sales[0].total, sales[0].margin), (17700, 6700));
        assert_eq!(store.products()?[0].stock, 2);
        store.refund(id)?;
        assert!(store.refund(id).is_err());
        assert_eq!(store.products()?[0].stock, 4);
        assert_eq!(store.products()?[1].stock, 1);
        assert!(store.sales()?[0].returned.is_some());
        Ok(())
    }

    #[test]
    fn money_uses_integer_kopiykas() -> Result<()> {
        assert_eq!(parse_money("49,9")?, 4990);
        assert_eq!(parse_money("0.01")?, 1);
        assert_eq!(money(-1), "−0,01 ₴");
        assert!(parse_money("49.001").is_err());
        assert!(parse_money("1000000,01").is_err());
        Ok(())
    }
}
