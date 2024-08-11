use crate::*;
use colored::Colorize;

#[derive(Clone)]
pub struct UserCtx {
  pub username: String,
  pub token: String,
  pub user_agent: String,
  pub establish_t: Time,
  pub session: Option<actix::Addr<WsSession>>,
}

impl UserCtx {
  fn hash(&self) -> String {
    HashGenerator::new(format!("{}{}{}", self.username, self.token, self.establish_t)).token() 
  }
}

impl PartialEq for UserCtx {
  fn eq(&self, other: &Self) -> bool {
    self.username == other.username
      && self.token == other.token
      && self.establish_t == other.establish_t
      && self.user_agent == other.user_agent
  }
}
impl Eq for UserCtx {}

impl fmt::Display for UserCtx {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    write!(
      f,
      "UserCtx{{ username: '{}', agent: '{}', create_at: '{}' }}",
      self.username, self.user_agent, self.establish_t
    )
  }
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct WsClient {
  username: String,
  user_ctx_hash: String,
}

impl WsClient {
  pub fn new(user_ctx: &UserCtx) -> Self {
    Self {
      username: user_ctx.username.clone(),
      user_ctx_hash: user_ctx.hash(),
    }
  }
}
#[derive(Message)]
#[rtype(result = "()")]
pub struct WsBinMessage(bytes::Bytes);

#[derive(Message)]
#[rtype(result = "()")]
pub struct WsTextMessage(String);

#[derive(Message)]
#[rtype(result = "()")]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct WsMessage {
  pub sender: WsSender,
  pub msg: WsMessageClass,
  pub policy: WsDispatchType,
}

#[derive(Message)]
#[rtype(result = "()")]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct WsMessageInner {
  pub sender: WsSender,
  pub msg: WsMessageClass,
  pub policy: WsDispatchType,
}

pub struct WsSession {
  pub server: Arc<Server>,
  pub hb_t: Time,
  pub user_ctx: UserCtx,
}


#[derive(serde::Deserialize, serde::Serialize)]
pub struct DashBoardInfo {
  pub online_user: u64,
  pub online_client: u64,
  pub user_used_storage: u64,
  pub user_max_storage: u64,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct HeartBeat {
  pub config: UserConfig,
  pub dashboard: DashBoardInfo,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct FileSendableResponse {
  pub file_elem: Option<FileListElem>,
  pub req: FileRequest,
  pub hashval: String,
  pub user_ctx_hash: String
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum WsMessageClass {
  HeartBeat(HeartBeat),       // two direction
  Establish,                  // two direction
  Reconnect,                  // from client
  CreateWsWorker(u64),        // two direction
  Leave,                      // on logout
  FileSendable(FileSendableResponse), // come out
  FileResponse(FileResponse), // two direction
  FileRequest(FileRequest),   // come in
  // filehash
  PleaseSend(String),         // come out
  Text(String),               // two direction
  Notify(String),
  Errjson(String),            // come out
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum WsDispatchType {
  Broadcast,
  BroadcastExceptMe,
  BroadcastSameUser,
  BroadcastSameUserExceptMe,
  Server,
  Targets(Vec<WsClient>),
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum WsSender {
  Server,
  User(WsClient),
  Manager(WsClient),
}

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(30);

use actix::prelude::*;
impl actix::Actor for WsSession {
  type Context = ws::WebsocketContext<Self>;
  fn started(&mut self, ctx: &mut Self::Context) {
    log::info!("actor started");
    ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
      if SystemTime::now()
        .duration_since(act.hb_t.system_time())
        .unwrap()
        > CLIENT_TIMEOUT
      {
        log::warn!("Websocket Client heartbeat failed, disconnecting!");
        ctx.stop();
        return;
      }
      ctx.ping(b""); // ping will send to Self
    });
  }

  fn stopping(&mut self, _ctx: &mut Self::Context) -> actix::Running {
    actix::Running::Stop
  }

  fn stopped(&mut self, _ctx: &mut Self::Context) {
    if self.user_ctx.session != None {
      assert!(
        self.server.w_remove_user_ctx(&self.user_ctx),
        "should have user_ctx"
      );
      log::info!("actor stopped");
    } else {
      log::info!("worker actor stopped");
    }
  }
}
impl Handler<WsTextMessage> for WsSession {
  type Result = ();

  // dispatch message
  fn handle(&mut self, text: WsTextMessage, ctx: &mut Self::Context) {
    ctx.text(text.0);
  }
}

impl Handler<WsMessage> for WsSession {
  type Result = ();

  // dispatch message
  fn handle(&mut self, ws_message: WsMessage, ctx: &mut Self::Context) {
    log::debug!(
      "handle wsmessage {}",
      serde_json::to_string(&ws_message).expect("ws message must be deserializable")
    );
    let pred: Box<dyn Fn(&UserCtx) -> bool>;
    match &ws_message.policy {
      WsDispatchType::Broadcast => {
        pred = Box::new(|_| true);
      }
      WsDispatchType::BroadcastExceptMe => {
        pred = Box::new(|user_ctx| user_ctx != &self.user_ctx);
      }
      WsDispatchType::BroadcastSameUser => {
        pred = Box::new(|user_ctx| {
          user_ctx.username == self.user_ctx.username
        });
      }
      WsDispatchType::BroadcastSameUserExceptMe => {
        pred = Box::new(|user_ctx| {
          user_ctx != &self.user_ctx && user_ctx.username == self.user_ctx.username
        });
      }
      WsDispatchType::Server => {
        ctx.address().do_send(WsMessageInner {
          sender: ws_message.sender,
          msg: ws_message.msg,
          policy: ws_message.policy,
        });
        return;
      }
      WsDispatchType::Targets(clients) => {
        pred = Box::new(|user_ctx| {
          for client in clients.iter() {
            if user_ctx.hash() == client.user_ctx_hash {
              return true;
            }
          }
          false
        });
      }
    }
    let user_ctxs = self.server.r_user_ctxs();
    for pair in user_ctxs.iter() {
      let ctx_vec = pair.1;
      for user_ctx in ctx_vec.iter() {
        // must send to self
        if pred(user_ctx) {
          let new_ws_message: WsMessage =
            serde_json::from_str(serde_json::to_string(&ws_message).unwrap().as_str()).unwrap();
          user_ctx.session.as_ref().unwrap().do_send(WsMessageInner {
            sender: new_ws_message.sender,
            msg: new_ws_message.msg,
            policy: WsDispatchType::Targets(vec![WsClient::new(user_ctx)]),
          });
        }
      }
    }
  }
}

impl Handler<WsMessageInner> for WsSession {
  type Result = ();

  fn handle(&mut self, ws_message: WsMessageInner, ctx: &mut Self::Context) {
    match &ws_message.policy {
      WsDispatchType::Server => (),
      WsDispatchType::Targets(clients) => {
        if clients.len() != 1 {
          panic!("unexpected");
        }
      }
      _ => {
        panic!("unexpected");
      }
    }

    match &ws_message.msg {
      WsMessageClass::HeartBeat(hb) => {
        let sqlhandler = SqlHandler::new(self.server.dbpool.clone());
        sqlhandler.update_user_config_by_name(&self.user_ctx.username, &hb.config).unwrap();
        let server_info = self.server.r_server_info();
        let sqlhandler = SqlHandler::new(self.server.dbpool.clone());
        let usertype = sqlhandler
          .get_user_by_name(&self.user_ctx.username)
          .expect("should has user")
          .expect("should has user")
          .usertype;
        let user_used_storage = 
            self.server.file_handler.get_user_used_storage(&self.user_ctx.username).unwrap();
        let user_max_storage = UserRight::from(usertype).max_storage;

        let send_hb = HeartBeat {
          config: hb.config.clone(),
          dashboard: DashBoardInfo {
            online_user: server_info.online_user,
            online_client: server_info.online_client,
            user_used_storage,
            user_max_storage
          }
        };

        ctx.address().do_send(WsTextMessage(
          serde_json::to_string(&WsMessage {
            sender: WsSender::Server,
            msg: WsMessageClass::HeartBeat(send_hb),
            policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
          })
          .unwrap(),
        ));
      }
      WsMessageClass::Reconnect => {
        log::info!("<- RECONNECT {}", serde_json::to_string(&ws_message).unwrap());
        match &ws_message.policy {
          WsDispatchType::Server => (),
          _ => {
            panic!("unexpected");
          }
        }
        let wsclient_before = match ws_message.sender {
          WsSender::User(u) => u,
          _ => {
            log::error!("unexpected");
            return;
          }
        };
        let username = wsclient_before.username;

        let sqlhandler = SqlHandler::new(self.server.dbpool.clone());
        let token = sqlhandler
          .get_user_by_name(&username)
          .expect("should has user")
          .expect("should has user")
          .token;
        self.user_ctx.token = token.clone();
        self.user_ctx.username = username.clone();
        self.user_ctx.session = Some(ctx.address());
        log::info!("reconnect add new user_ctx: {}", self.user_ctx);
        self.server.w_add_user_ctx(self.user_ctx.clone());
        ctx.address().do_send(WsTextMessage(
          serde_json::to_string(&WsMessage {
            sender: WsSender::Server,
            msg: WsMessageClass::Reconnect,
            policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
          })
          .unwrap(),
        ));
      }
      WsMessageClass::Establish => {
        log::info!("<- ESTABLISH {}", serde_json::to_string(&ws_message).unwrap());
        match &ws_message.policy {
          WsDispatchType::Server => (),
          _ => {
            panic!("unexpected");
          }
        }

        let username = match ws_message.sender {
          WsSender::User(u) => u.username.clone(),
          _ => {
            log::error!("unexpected");
            return;
          }
        };
        let sqlhandler = SqlHandler::new(self.server.dbpool.clone());
        let token = sqlhandler
          .get_user_by_name(&username)
          .expect("should has user")
          .expect("should has user")
          .token;

        self.user_ctx.token = token.clone();
        self.user_ctx.username = username.clone();
        self.user_ctx.session = Some(ctx.address());
        log::info!("add new user_ctx: {}", self.user_ctx);
        self.server.w_add_user_ctx(self.user_ctx.clone());

        // manager login will broadcast to all clients
        if self.server.r_is_manager(&username) {
          ctx.address().do_send(WsMessage {
            sender: WsSender::Manager(WsClient::new(&self.user_ctx)),
            msg: WsMessageClass::Notify("Enter the site!".into()),
            policy: WsDispatchType::BroadcastExceptMe,
          });
        }
        // user login will broadcast to all clients with same user
        else {
          ctx.address().do_send(WsMessage {
            sender: WsSender::User(WsClient::new(&self.user_ctx)),
            msg: WsMessageClass::Notify("your account login at another place!".into()),
            policy: WsDispatchType::BroadcastSameUserExceptMe,
          });
        }

        let msg = 
          serde_json::to_string(&WsMessage {
            sender: WsSender::Server,
            msg: WsMessageClass::Establish,
            policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
          })
          .unwrap();
        log::info!("-> ESTABLISH {}", msg);
        ctx.text(msg);
      }
      WsMessageClass::Leave => {
        log::info!("<- LEAVE {}", serde_json::to_string(&ws_message).unwrap());
        match &ws_message.policy {
          WsDispatchType::Server => (),
          _ => {
            panic!("unexpected");
          }
        }

        if self.server.r_is_manager(&self.user_ctx.username) {
          ctx.address().do_send(WsMessage {
            sender: WsSender::Manager(WsClient::new(&self.user_ctx)),
            msg: WsMessageClass::Notify("Leave the site!".into()),
            policy: WsDispatchType::BroadcastExceptMe,
          });
        }
        // user login will broadcast to all clients with same user
        else {
          ctx.address().do_send(WsMessage {
            sender: WsSender::User(WsClient::new(&self.user_ctx)),
            msg: WsMessageClass::Notify("your account leave at another place!".into()),
            policy: WsDispatchType::BroadcastSameUserExceptMe,
          });
        }

        let msg =
          serde_json::to_string(&WsMessage {
            sender: WsSender::Server,
            msg: WsMessageClass::Leave,
            policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
          })
          .unwrap();
        log::info!("-> LEAVE {}", msg);
        ctx.text(msg);
      }
      WsMessageClass::FileRequest(pkg) => {
        log::info!("<- FILE REQUEST {}", serde_json::to_string(&ws_message).unwrap());
        let sqlhandler = SqlHandler::new(self.server.dbpool.clone());
        let usertype = sqlhandler
          .get_user_by_name(&self.user_ctx.username)
          .expect("should has user")
          .expect("should has user")
          .usertype;
        let user_used_storage = 
            self.server.file_handler.get_user_used_storage(&self.user_ctx.username).unwrap();
        let user_max_storage = UserRight::from(usertype).max_storage;

        let can = pkg.size + user_used_storage <= user_max_storage && 
            self.server.file_handler.add(pkg.clone(), self.user_ctx.clone());
        let mut file_sendable_resp = FileSendableResponse {
          file_elem: None,
          hashval: pkg.file_hash.clone(),
          req: pkg.clone(),
          user_ctx_hash: self.user_ctx.hash()
        };
        if can {
          match FileListElem::from(pkg.username.clone(), pkg.name.clone(), pkg.size) {
            Ok(file_elem) => {
              file_sendable_resp.file_elem = Some(file_elem);
            }
            Err(e) => {
              log::error!("get file elem error: {}", e.to_string());
            }
          };
        }
        let msg = WsMessage {
          sender: WsSender::Server,
          msg: WsMessageClass::FileSendable(file_sendable_resp),
          policy: WsDispatchType::BroadcastSameUser,
        };
        log::info!("->* FILESENDABLE {}", serde_json::to_string(&msg).unwrap());
        ctx.address().do_send(msg);
      }
      WsMessageClass::Text(_) => {
        ctx.address().do_send(WsTextMessage(serde_json::to_string(&ws_message).unwrap()));
      }
      WsMessageClass::Errjson(e) => {
        log::error!("ERR JSON: {e}");
      }
      WsMessageClass::Notify(_) => {
        ctx.address().do_send(WsTextMessage(serde_json::to_string(&ws_message).unwrap()));
      }
      WsMessageClass::FileResponse(resp) => {
        match &ws_message.policy {
          WsDispatchType::Targets(_) => {
            ctx.text(serde_json::to_string(&ws_message).unwrap());
            log::info!(
              "-> FILE RESPONSE {} {} {:?}", resp.name.green(), resp.file_hash, resp.slice_idx);
          }
          WsDispatchType::Server => {
            log::info!(
              "<- FILE FINISH {} {} {:?}", resp.name.green(), resp.file_hash, resp.slice_idx);
            match &resp.status {
              FileResponseStatus::Finish => (),
              _ => assert!(false)
            }
            self.server.file_handler.done(resp.file_hash.clone());
            log::info!(
              "->* FILE FINISH {} {} {:?}", resp.name.green(), resp.file_hash, resp.slice_idx);
            // tell every client send is finish
            ctx.address().do_send(WsMessage {
              sender: WsSender::Server,
              msg: ws_message.msg,
              policy: WsDispatchType::BroadcastSameUser
            });
          }
          _ => ()
        }
      }
      WsMessageClass::FileSendable(_) => {
        let msg = serde_json::to_string(&ws_message).unwrap();
        log::info!("-> FILE SENDABLE {}", msg);
        ctx.text(msg);
      }
      WsMessageClass::PleaseSend(_) => {
        let msg = serde_json::to_string(&ws_message).unwrap();
        log::warn!("-> PLEASE SEND {}", msg);
        ctx.text(msg);
      }
      WsMessageClass::CreateWsWorker(id) => {
        let msg = 
          serde_json::to_string(&WsMessage {
            sender: WsSender::Server,
            msg: WsMessageClass::CreateWsWorker(*id),
            policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
          })
          .unwrap();
        log::info!("-> CREATEWSWORKER {}", msg);
        ctx.text(msg);
      }
    }
  }
}

impl Handler<WsBinMessage> for WsSession {
  type Result = ();
  fn handle(&mut self, b: WsBinMessage, _ctx: &mut Self::Context) {
    self.server.file_handler.send(b.0);
  }
}

impl actix::StreamHandler<Result<ws::Message, ws::ProtocolError>> for WsSession {
  fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
    let msg = match msg {
      Err(e) => {
        log::error!("ws msg is Err: {}", e);
        return;
      }
      Ok(msg) => msg,
    };
    match msg {
      ws::Message::Ping(msg) => {
        self.hb_t = Time::now();
        log::debug!("ws: {:?}", msg);
        ctx.pong(&msg);
      }
      ws::Message::Pong(_) => {
        self.hb_t = Time::now();
      }
      ws::Message::Text(text) => {
        // new client connected
        log::debug!("ws receive text from client: {}", text);
        let ws_message: WsMessage = match serde_json::from_str(&text) {
          Ok(m) => m,
          Err(e) => {
            ctx.address().do_send(WsTextMessage(
              serde_json::to_string(&WsMessage {
                sender: WsSender::Server,
                msg: WsMessageClass::Errjson(e.to_string()),
                policy: WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]),
              })
              .unwrap(),
            ));
            return;
          }
        };
        // send Self for more function
        ctx.address().do_send(ws_message);
      }
      ws::Message::Binary(b) => {
        ctx.address().do_send(WsBinMessage(b));
      }
      ws::Message::Close(reason) => {
        log::info!("ws receive close: {:?}", reason);
        ctx.close(reason);
        ctx.stop();
      }
      ws::Message::Continuation(item) => {
        let bys: bytes::Bytes;
        match item {
          actix_http::ws::Item::Continue(bytes) => {
            bys = bytes;
            log::warn!("CONTINUATION CONTINUE");
          }
          actix_http::ws::Item::FirstText(bytes) => {
            bys = bytes;
            log::warn!("CONTINUATION FirstText");
          }
          actix_http::ws::Item::FirstBinary(bytes) => {
            bys = bytes;
            log::warn!("CONTINUATION FirstBinary");
          }
          actix_http::ws::Item::Last(bytes) => {
            bys = bytes;
            log::warn!("CONTINUATION Last");
          }
        }
        let hashstr: String = bys.slice(0..32).iter().map(|b| {
          format!("{:02x}", b).to_string()
        }).collect();
        let index: u64 = bys.slice(32..36).get_u32_le() as u64;
        log::warn!("hash {}, index {}, len {}", hashstr, index, bys.len());
        ctx.stop();
      }
      ws::Message::Nop => {}
    }
  }
}
