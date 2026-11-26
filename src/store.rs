use anyhow::{Context, Result, ensure};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use rand_core::OsRng;
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

pub type Cart = BTreeMap<i64, (i64, i64)>;
pub type ParkedRef = (i64, i64);

pub struct ParkedCart {
    pub id: i64,
    pub customer: Option<i64>,
    pub units: i64,
    pub total: i64,
    pub saved_at: String,
}

#[derive(Clone, Copy, Default)]
pub enum Tender {
    #[default]
    Cash,
    Card,
    Split {
        cash: i64,
    },
}

#[derive(Clone, Copy, Default)]
pub struct Payment {
    pub tender: Tender,
    pub redeemed: i64,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PaymentBreakdown {
    pub cash: i64,
    pub card: i64,
    pub redeemed: i64,
    pub earned: i64,
}

impl Payment {
    pub fn quote(self, gross: i64, balance: i64, has_customer: bool) -> Result<PaymentBreakdown> {
        ensure!(
            self.redeemed >= 0 && self.redeemed <= gross,
            "Бонуси перевищують суму кошика"
        );
        ensure!(
            self.redeemed == 0 || (has_customer && self.redeemed <= balance),
            "Недостатньо бонусів"
        );
        let due = gross - self.redeemed;
        let (cash, card) = match self.tender {
            Tender::Cash => (due, 0),
            Tender::Card => (0, due),
            Tender::Split { cash } => {
                ensure!(
                    cash > 0 && cash < due,
                    "Для змішаної оплати вкажіть готівку менше суми до сплати"
                );
                (cash, due - cash)
            }
        };
        let earned = if has_customer { due / 100 } else { 0 };
        Ok(PaymentBreakdown {
            cash,
            card,
            redeemed: self.redeemed,
            earned,
        })
    }
}

pub struct Sale {
    pub id: i64,
    pub created: String,
    pub returned: Option<String>,
    pub customer: Option<String>,
    pub total: i64,
    pub margin: i64,
    pub payment: Option<PaymentBreakdown>,
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

fn valid_pin(pin: &str) -> bool {
    (6..=12).contains(&pin.len()) && pin.bytes().all(|byte| byte.is_ascii_digit())
}

fn hash_pin(pin: &str) -> Result<String> {
    ensure!(valid_pin(pin), "PIN має містити від 6 до 12 цифр");
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(pin.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| anyhow::anyhow!("Не вдалося зберегти PIN: {error}"))
}

fn matches_pin(pin: &str, stored: &str) -> Result<bool> {
    let hash =
        PasswordHash::new(stored).map_err(|error| anyhow::anyhow!("Пошкоджений PIN: {error}"))?;
    Ok(Argon2::default()
        .verify_password(pin.as_bytes(), &hash)
        .is_ok())
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Self(conn))
    }

    pub fn has_admin_pin(&self) -> Result<bool> {
        Ok(self
            .0
            .query_row("SELECT EXISTS(SELECT 1 FROM admin_access)", [], |row| {
                row.get(0)
            })?)
    }

    pub fn set_initial_admin_pin(&self, pin: &str) -> Result<()> {
        let hash = hash_pin(pin)?;
        ensure!(
            self.0.execute(
                "INSERT INTO admin_access(id,pin_hash) VALUES(1,?1) ON CONFLICT(id) DO NOTHING",
                [&hash],
            )? == 1,
            "PIN адміністратора вже встановлено"
        );
        Ok(())
    }

    pub fn verify_admin_pin(&self, pin: &str) -> Result<bool> {
        if !valid_pin(pin) {
            return Ok(false);
        }
        let stored: String =
            self.0
                .query_row("SELECT pin_hash FROM admin_access WHERE id=1", [], |row| {
                    row.get(0)
                })?;
        Ok(matches_pin(pin, &stored)?)
    }

    pub fn change_admin_pin(&mut self, current: &str, new: &str) -> Result<()> {
        let tx = self.0.transaction_with_behavior(Immediate)?;
        let stored: String =
            tx.query_row("SELECT pin_hash FROM admin_access WHERE id=1", [], |row| {
                row.get(0)
            })?;
        ensure!(
            valid_pin(current) && matches_pin(current, &stored)?,
            "Неправильний поточний PIN"
        );
        tx.execute(
            "UPDATE admin_access SET pin_hash=?1 WHERE id=1",
            [hash_pin(new)?],
        )?;
        tx.commit()?;
        Ok(())
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

    pub fn customers(&self) -> Result<Vec<(i64, String)>> {
        let mut statement = self
            .0
            .prepare("SELECT id,name FROM customer ORDER BY name COLLATE NOCASE,id")?;
        Ok(statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn create_customer(&self, name: &str) -> Result<i64> {
        let name = name.trim();
        ensure!(
            (1..=100).contains(&name.chars().count()),
            "Вкажіть ім’я клієнта (до 100 символів)"
        );
        Ok(self.0.query_row(
            "INSERT INTO customer(name) VALUES(?1) RETURNING id",
            [name],
            |row| row.get(0),
        )?)
    }

    pub fn parked_carts(&self) -> Result<Vec<ParkedCart>> {
        let mut statement = self.0.prepare(
            "SELECT pc.id,pc.customer,SUM(pl.quantity),SUM(pl.quantity*pl.price),pc.saved_at
             FROM parked_cart pc JOIN parked_line pl ON pl.cart=pc.id
             GROUP BY pc.id ORDER BY pc.saved_at DESC,pc.id DESC",
        )?;
        Ok(statement
            .query_map([], |row| {
                Ok(ParkedCart {
                    id: row.get(0)?,
                    customer: row.get(1)?,
                    units: row.get(2)?,
                    total: row.get(3)?,
                    saved_at: row.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub fn park_cart(
        &mut self,
        cart: &Cart,
        customer: Option<i64>,
        parked: Option<ParkedRef>,
    ) -> Result<i64> {
        ensure!(!cart.is_empty(), "Кошик порожній");
        let tx = self.0.transaction_with_behavior(Immediate)?;
        let id = if let Some((id, revision)) = parked {
            ensure!(tx.execute(
                "UPDATE parked_cart SET revision=revision+1,customer=?3,saved_at=CURRENT_TIMESTAMP
                 WHERE id=?1 AND revision=?2", params![id, revision, customer],
            )? == 1, "Відкладений кошик уже змінено або завершено");
            tx.execute("DELETE FROM parked_line WHERE cart=?1", [id])?;
            id
        } else {
            tx.query_row(
                "INSERT INTO parked_cart(customer) VALUES(?1) RETURNING id",
                [customer],
                |row| row.get(0),
            )?
        };
        {
            let mut insert = tx.prepare(
                "INSERT INTO parked_line(cart,product,quantity,price) VALUES(?1,?2,?3,?4)",
            )?;
            for (&product, &(quantity, price)) in cart {
                insert.execute(params![id, product, quantity, price])?;
            }
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn load_parked_cart(&mut self, id: i64) -> Result<(ParkedRef, Option<i64>, Cart)> {
        let tx = self.0.transaction()?;
        let (revision, customer) = tx
            .query_row(
                "SELECT revision,customer FROM parked_cart WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .context("Відкладений кошик недоступний")?;
        let cart = {
            let mut query =
                tx.prepare("SELECT product,quantity,price FROM parked_line WHERE cart=?1")?;
            query
                .query_map([id], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?
                .collect::<rusqlite::Result<Cart>>()?
        };
        tx.commit()?;
        Ok(((id, revision), customer, cart))
    }

    pub fn customer_bonus(&self, customer: i64) -> Result<i64> {
        customer_bonus(&self.0, customer)
    }

    pub fn sell(
        &mut self,
        cart: &Cart,
        customer: Option<i64>,
        parked: Option<ParkedRef>,
        payment: Payment,
    ) -> Result<i64> {
        ensure!(!cart.is_empty(), "Кошик порожній");
        let tx = self.0.transaction_with_behavior(Immediate)?;
        let gross = cart
            .values()
            .try_fold(0_i64, |sum, &(quantity, price)| {
                quantity
                    .checked_mul(price)
                    .and_then(|amount| sum.checked_add(amount))
            })
            .context("Сума кошика завелика")?;
        let balance = customer
            .map(|id| customer_bonus(&tx, id))
            .transpose()?
            .unwrap_or_default();
        let breakdown = payment.quote(gross, balance, customer.is_some())?;
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
        if let Some(customer) = customer {
            tx.execute(
                "INSERT INTO sale_customer(sale,customer) VALUES(?1,?2)",
                params![id, customer],
            )?;
        }
        if let Some((id, revision)) = parked {
            ensure!(
                tx.execute(
                    "DELETE FROM parked_cart WHERE id=?1 AND revision=?2",
                    params![id, revision]
                )? == 1,
                "Відкладений кошик уже змінено або завершено"
            );
        }
        tx.execute(
            "INSERT INTO sale_payment(sale,cash,card,redeemed,earned) VALUES(?1,?2,?3,?4,?5)",
            params![
                id,
                breakdown.cash,
                breakdown.card,
                breakdown.redeemed,
                breakdown.earned
            ],
        )?;
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
            "SELECT sale.id,created,returned_at,SUM(quantity*unit_price)-COALESCE(p.redeemed,0),
                    SUM(quantity*(unit_price-unit_cost))-COALESCE(p.redeemed,0),customer.name,
                    p.cash,p.card,p.redeemed,p.earned
             FROM sale JOIN sale_line ON sale.id=sale_line.sale
             LEFT JOIN sale_customer ON sale_customer.sale=sale.id
             LEFT JOIN customer ON customer.id=sale_customer.customer
             LEFT JOIN sale_payment p ON p.sale=sale.id
             GROUP BY sale.id ORDER BY sale.id DESC",
        )?;
        Ok(q.query_map([], |r| {
            let payment = if let Some(cash) = r.get::<_, Option<i64>>(6)? {
                Some(PaymentBreakdown {
                    cash,
                    card: r.get(7)?,
                    redeemed: r.get(8)?,
                    earned: r.get(9)?,
                })
            } else {
                None
            };
            Ok(Sale {
                id: r.get(0)?,
                created: r.get(1)?,
                returned: r.get(2)?,
                total: r.get(3)?,
                margin: r.get(4)?,
                customer: r.get(5)?,
                payment,
            })
        })?
        .collect::<rusqlite::Result<_>>()?)
    }
}

fn customer_bonus(conn: &Connection, id: i64) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COALESCE(SUM(p.earned-p.redeemed),0)
         FROM sale_customer c JOIN sale s ON s.id=c.sale
         JOIN sale_payment p ON p.sale=s.id WHERE c.customer=?1 AND s.returned_at IS NULL",
        [id],
        |row| row.get(0),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_tender_and_bonus_are_committed_and_reversed_with_sale() -> Result<()> {
        let mut store = Store::open(Path::new(":memory:"))?;
        store.add_demo_products()?;
        let p = store.products()?.remove(0);
        let customer = store.create_customer("Олена")?;
        let cart = Cart::from([(p.id, (1, p.price))]);
        let first = store.sell(&cart, Some(customer), None, Payment::default())?;
        let balance = p.price / 100;
        assert_eq!(store.customer_bonus(customer)?, balance);
        let mixed = Payment {
            tender: Tender::Split { cash: 1 },
            redeemed: balance,
        };
        assert!(store.sell(&cart, None, None, mixed).is_err());
        let second = store.sell(&cart, Some(customer), None, mixed)?;
        let sales = store.sales()?;
        let due = p.price - balance;
        assert_eq!(sales[0].total, due);
        assert_eq!(
            sales[0].payment,
            Some(PaymentBreakdown {
                cash: 1,
                card: due - 1,
                redeemed: balance,
                earned: due / 100
            })
        );
        assert_eq!(store.customer_bonus(customer)?, due / 100);
        store.refund(second)?;
        assert_eq!(store.customer_bonus(customer)?, balance);
        store.refund(first)?;
        assert_eq!(store.customer_bonus(customer)?, 0);
        Ok(())
    }

    #[test]
    fn parked_cart_keeps_customer_and_stock_until_sale() -> Result<()> {
        let mut store = Store::open(Path::new(":memory:"))?;
        store.add_demo_products()?;
        let product = store.products()?.remove(0);
        let customer = store.create_customer("Олена")?;
        let cart = Cart::from([(product.id, (2, product.price))]);
        let id = store.park_cart(&cart, Some(customer), None)?;
        assert_eq!(store.products()?[0].stock, product.stock);
        let (first, selected, restored) = store.load_parked_cart(id)?;
        assert_eq!(selected, Some(customer));
        assert_eq!(restored, cart);
        assert_eq!(store.park_cart(&cart, selected, Some(first))?, id);
        assert!(
            store
                .sell(&cart, selected, Some(first), Payment::default())
                .is_err()
        );
        assert_eq!(store.products()?[0].stock, product.stock);
        let (current, _, _) = store.load_parked_cart(id)?;
        store.sell(&cart, selected, Some(current), Payment::default())?;
        assert_eq!(store.sales()?[0].customer.as_deref(), Some("Олена"));
        assert_eq!(store.products()?[0].stock, product.stock - 2);
        assert!(store.parked_carts()?.is_empty());
        Ok(())
    }

    #[test]
    fn admin_pin_is_initialized_once_and_can_be_changed() -> Result<()> {
        let mut store = Store::open(Path::new(":memory:"))?;
        assert!(!store.has_admin_pin()?);
        assert!(store.set_initial_admin_pin("123").is_err());
        store.set_initial_admin_pin("123456")?;
        assert!(store.has_admin_pin()?);
        assert!(store.set_initial_admin_pin("654321").is_err());
        assert!(!store.verify_admin_pin("654321")?);
        assert!(store.change_admin_pin("654321", "999999").is_err());
        store.change_admin_pin("123456", "654321")?;
        assert!(!store.verify_admin_pin("123456")?);
        assert!(store.verify_admin_pin("654321")?);
        Ok(())
    }

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
        assert!(
            store
                .sell(&invalid, None, None, Payment::default())
                .is_err()
        );
        assert!(store.sales()?.is_empty());
        assert_eq!(store.products()?[0].stock, 4);

        store.count(tea.id, 1)?;
        let id = store.sell(&invalid, None, None, Payment::default())?;
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
