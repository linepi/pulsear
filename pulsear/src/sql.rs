use crate::*;

/// should only be used by one thread
pub struct SqlHandler {
  dbpool: mysql::Pool,
}

impl SqlHandler {
  pub fn new(dbpool: mysql::Pool) -> Self {
    Self {
      dbpool
    }
  }

  pub fn dbpool(&self) -> &mysql::Pool {
    &self.dbpool
  }
  /// prerequisite: user_config table created
  /// returned users: with all field filled
  pub fn get_users(&self) -> Result<Vec<User>, Err> {
    let mut dbconn = self.dbpool.get_conn()?;
    let mut users: Vec<User> = vec![];
    let stmt = dbconn.prep(
      r"SELECT user.id, username, token, theme, user_config.id, web_worker_num, filelist_config, type
			  from user, user_config 
			  where user.id = user_config.user_id",
    )?;
    let rows: Vec<mysql::Row> = dbconn.exec(&stmt, ())?;
    for r in rows {
      let elems: (i32, String, String, String, i32, i32, String, String) = 
          mysql::from_row_opt(r)?;
      let user = User {
        id: elems.0,
        username: elems.1,
        token: elems.2,
        config: UserConfig {
          id: elems.4,
          theme: elems.3,
          web_worker_num: elems.5,
          filelist_config: serde_json::from_str(&elems.6).unwrap()
        },
        usertype: serde_json::from_str(&elems.7)?
      };
      users.push(user);
    }
    Ok(users)
  }

  /// prerequisity: user_config table created
  /// returned user: with all field filled
  pub fn get_user_by_name(
    &self,
    username: &String,
  ) -> Result<Option<User>, Err> {
    let mut dbconn = self.dbpool.get_conn()?;
    let stmt = dbconn.prep(
      r"SELECT user.id, username, token, theme, user_config.id, web_worker_num, filelist_config, type
			  from user, user_config 
			  where user.id = user_config.user_id and
				    username = :name",
    )?;
    let rows: Vec<mysql::Row> = dbconn.exec(&stmt, params! { "name" => &username })?;
    if rows.len() == 0 {
      return Ok(None);
    } else if rows.len() > 1 {
      return Err(Box::from("multiple use found"));
    }
    let row: (i32, String, String, String, i32, i32, String, String) =
      mysql::from_row_opt(rows.first().unwrap().to_owned())?;
    let user = User {
      id: row.0,
      username: row.1,
      token: row.2,
      config: UserConfig {
        id: row.4,
        theme: row.3,
        web_worker_num: row.5,
        filelist_config: serde_json::from_str(&row.6)?
      },
      usertype: UserType::from(&row.7)
    };
    Ok(Some(user))
  }

  /// user: username, token, config
  /// returned user: id, ..., config_id
  pub fn add_user(&self, user: &User) -> Result<Option<User>, Err> {
    let mut dbconn = self.dbpool.start_transaction(TxOpts::default())?;
    let stmt = dbconn.prep(
      r"INSERT INTO user(username, token, type)
			  VALUES (:username, :token, :type)",
    )?;
    dbconn.exec_drop(
      &stmt,
      params! { 
        "username" => &user.username, 
        "token" => &user.token,
        "type" => &user.usertype.to_string() },
    )?;
    let user_id: i32 = dbconn
      .exec_first(
        r"SELECT id from user
			  WHERE username = ?",
        (&user.username,),
      )?
      .expect("user should exists after insertion");

    let stmt = dbconn.prep(
      r"INSERT INTO user_config(user_id, theme, web_worker_num, filelist_config)
			  VALUES (:user_id, :theme, :web_worker_num, :filelist_config)",
    )?;
    dbconn.exec_drop(
      &stmt,
      params! { 
        "user_id" => &user_id, 
        "theme" => &user.config.theme,
        "web_worker_num" => user.config.web_worker_num,
        "filelist_config" => serde_json::to_string(&user.config.filelist_config)?
      },
    )?;
    dbconn.commit()?;
    self.get_user_by_name(&user.username)
  }

  pub fn delete_user_by_name(&self, username: &String) -> Result<(), Err> {
    match self.get_user_by_name(username)? {
      Some(u) => log::info!("delete user[{:?}]", u),
      None => return Err(Box::from(format!("user does not exist: {}", username))),
    }

    let mut dbconn = self.dbpool.start_transaction(TxOpts::default())?;
    dbconn.exec_drop(
      r"DELETE FROM user_config 
			  WHERE user_id = (
			  	SELECT id FROM user
				WHERE username = ?
			  )",
      (username,),
    )?;
    dbconn.exec_drop(
      r"DELETE FROM user 
			  WHERE username = ?",
      (username,),
    )?;
    dbconn.commit()?;
    Ok(())
  }

  pub fn update_user_config_by_name(
    &self,
    username: &String,
    config: &UserConfig,
  ) -> Result<(), Err> {
    match self.get_user_by_name(username)? {
      Some(u) => log::debug!("update user[{:?}]'s config as {:?}", u, config),
      None => return Err(Box::from(format!("user does not exist: {}", username))),
    }
    let mut dbconn = self.dbpool.get_conn()?;
    dbconn.exec_drop(
      r"UPDATE user_config SET theme=?, web_worker_num=?, filelist_config=?
			  WHERE user_id = (
			  	SELECT id FROM user
				WHERE username = ?
			  )",
      (&config.theme, &config.web_worker_num, 
       serde_json::to_string(&config.filelist_config)?, &username),
    )?;
    Ok(())
  }

  /// change last login time
  pub fn user_login(&self, username: &String) -> Result<(), Err> {
    let mut dbconn = self.dbpool.get_conn()?;
    dbconn.exec_drop(
      r"UPDATE user SET last_login_time=NOW()
			  WHERE username = ?",
      (&username,),
    )?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn mysql_conn() -> std::result::Result<(), Err> {
    if let Ok(url) = std::env::var("PULSEAR_DATABASE_URL") {
      let pool = mysql::Pool::new(url.as_str())?;
      let _ = pool.get_conn()?;
    } else {
      return Err(Box::from("please set env PLUSEAR_DATABASE_URL"));
    }
    Ok(())
  }

  #[test]
  fn sqlhandler() -> Result<(), Err> {
    if let Ok(url) = std::env::var("PULSEAR_DATABASE_URL") {
      let handler = SqlHandler {
        dbpool: mysql::Pool::new(url.as_str())?,
      };
      let ul: Vec<String> = 
          vec!["userggh0", "userggh1", "userggh2"].iter().map(|s| s.to_string()).collect();
      let tk: Vec<String> = 
          vec!["userggh0", "userggh1", "userggh2"].iter().map(|s| s.to_string()).collect();
      for name in ul.iter() {
        handler.delete_user_by_name(&name.to_string()).unwrap_or(());
        assert!(handler.get_user_by_name(&name.to_string())?.is_none());
      }

      // basic
      handler.add_user(&User {
          id: 0,
          username: ul[0].clone(),
          token: tk[0].clone(),
          config: UserConfig::default(),
          usertype: UserType::default()
        })?
        .unwrap();
      assert!(handler.get_user_by_name(&ul[0])?.is_some());
      let user = handler.get_user_by_name(&ul[0])?.unwrap();
      assert_eq!(&ul[0], &user.username);
      assert_eq!(&tk[0], &user.token);
      assert_eq!(&UserConfig::default(), &user.config);

      // change config
      let mut config = UserConfig::default();
      config.theme = "light".to_string();
      handler.update_user_config_by_name(
        &ul[0], &config
      )?;
      let user0 = handler.get_user_by_name(&ul[0])?.unwrap();
      assert_eq!(&ul[0], &user0.username);
      assert_eq!(&ul[0], &user0.token);
      assert_eq!("light", &user0.config.theme);

      handler
        .add_user(&User {
          id: 0,
          username: ul[1].clone(),
          token: tk[1].clone(),
          config: UserConfig::default(),
          usertype: UserType::default()
        })?
        .unwrap();
      assert!(handler.get_user_by_name(&ul[1])?.is_some());

      assert_eq!(
        handler
          .get_users()?
          .iter()
          .filter(|u| { *u.username == ul[0] || *u.username == ul[1] })
          .count(),
        2
      );

      handler.delete_user_by_name(&ul[0])?;
      assert_eq!(
        handler
          .get_users()?
          .iter()
          .filter(|u| { *u.username == ul[0] || *u.username == ul[1] })
          .count(),
        1
      );

      handler.delete_user_by_name(&ul[1])?;
      assert_eq!(
        handler
          .get_users()?
          .iter()
          .filter(|u| { *u.username == ul[0] || *u.username == ul[1] })
          .count(),
        0
      );
    } else {
      return Err(Box::from("please set env PLUSEAR_DATABASE_URL"));
    }
    Ok(())
  }
}
