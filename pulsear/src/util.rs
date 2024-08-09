use crate::*;

#[derive(Hash)]
pub struct HashGenerator {
  s: String,
}

impl HashGenerator {
  pub fn new(s: String) -> Self {
    Self {
      s
    }
  }

  pub fn token(&self) -> String {
    use std::hash::{DefaultHasher, Hasher};
    let mut hasher = DefaultHasher::new();
    self.hash(&mut hasher);
    hasher.finish().to_string()
  }
}

#[derive(Clone)]
pub struct Time(SystemTime);
impl Time {
  pub fn now() -> Self {
    Self(SystemTime::now())
  }

  pub fn from(st: SystemTime) -> Self {
    Self(st)
  }

  pub fn milli(&self) -> u64 {
    let since_the_epoch = self
      .0
      .duration_since(std::time::UNIX_EPOCH)
      .expect("Time went backwards");
    since_the_epoch.as_millis() as u64
  }

  pub fn nano(&self) -> u64 {
    let since_the_epoch = self
      .0
      .duration_since(std::time::UNIX_EPOCH)
      .expect("Time went backwards");
    since_the_epoch.as_nanos() as u64
  }

  pub fn system_time(&self) -> SystemTime {
    self.0
  }

  pub fn as_fmt(&self, fmt: &str) -> String {
    use chrono::DateTime;
    let datetime: DateTime<chrono::Local> = self.0.into();
    format!("{}", datetime.format(fmt))
  }
}

impl PartialEq for Time {
  fn eq(&self, other: &Self) -> bool {
    self.nano() == other.nano()
  }
}

impl fmt::Display for Time {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    use chrono::DateTime;
    let datetime: DateTime<chrono::Local> = self.0.into();
    write!(f, "{}", datetime.to_rfc2822())
  }
}


#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashSet;
  #[test]
  fn basic_test() -> Result<(), Err> {
    let set = HashSet::from([1, 3, 21]);
    let content: Vec<&i32> = set.iter().collect();
    assert!(content.contains(&&1));
    assert!(content.contains(&&3));
    assert!(content.contains(&&21));
    let mut map = HashMap::<i32, i32>::new();
    assert!(map.insert(3, 4).is_none());
    assert!(map.insert(3, 4).is_some());
    assert_eq!(map.get(&3).unwrap(), &4);
    Ok(())
  }

  fn unit() -> Result<(), Err> {
    let _time = Time::now();
    Ok(())
  }
}
