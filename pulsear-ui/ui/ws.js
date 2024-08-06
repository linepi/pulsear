/**
struct WsMessage {
  sender: WsSender,
  msg: WsMessageClass,
  policy: WsDispatchType,
}
*/

function pclone(obj) {
  return JSON.parse(JSON.stringify(obj));
}

class WsClient {
  #username
  #user_ctx_hash
  constructor(name, hash) {
    this.#username = name;
    this.#user_ctx_hash = hash;
  }

  asObj() {
    return { 
      username: pclone(this.#username),
      user_ctx_hash: pclone(this.#user_ctx_hash) 
    }
  }

  static fromObj(obj) {
    return new WsClient(obj.username, obj.user_ctx_hash);
  }

  static fromJson(json) {
    const obj = JSON.parse(json);
    return this.fromObj(obj);
  }

  toJson() {
    return JSON.stringify(this.asObj());
  }

  equals(other) {
    if (!(other instanceof WsClient)) {
      throw new Error("compare only work in same type");
    }
    return this.toJson() === other.toJson();
  }

  get username() {
    return this.#username;
  }

  set username(name) {
    if (typeof name !== "string") {
      throw new Error("type should be string");
    }
    this.#username = name;
  }

  get user_ctx_hash() {
    return this.#user_ctx_hash;
  }

  set user_ctx_hash(hash) {
    if (typeof hash !== "string") {
      throw new Error("type should be string");
    }
    this.#user_ctx_hash = hash;
  }
}

class WsSender {
  static Server = new WsSender(0, null);
  static User = new WsSender(1, null);
  static Manager = new WsSender(2, null);
  static withUser(name, hash) {
    // see WsClient
    return new WsSender(1, new WsClient(name, hash).asObj());
  }
  static withManager(name, hash) {
    return new WsSender(2, new WsClient(name, hash).asObj());
  }
  #value;
  #wsclient;

  constructor(val, wsclient) {
    this.#value = val;
    this.#wsclient = wsclient;
  }

  get wsclient() {
    return this.#wsclient;
  }

  set wsclient(wsc) {
    if (wsc && !(wsc instanceof WsClient)) {
      throw new Error("type not true");
    }
    this.#wsclient = wsc;
  }

  asObj() {
    let out_obj;
    switch (this.#value) {
      case 0:
        out_obj = "Server";
        break;
      case 1:
        out_obj = { User: pclone(this.#wsclient) };
        break;
      case 2:
        out_obj = { Manager: pclone(this.#wsclient) };
        break;
      default:
        throw new Error("unexpected");
    }
    return out_obj;
  }

  static fromObj(obj) {
    if (obj === "Server") {
      return WsSender.Server;
    } else if (obj.User && typeof obj.User.username === "string" &&
      typeof obj.User.user_ctx_hash === "string") {
      return WsSender.withUser(obj.User.username, obj.User.user_ctx_hash);
    } else if (obj.Manager && typeof obj.Manager.username === "string" &&
      typeof obj.Manager.user_ctx_hash === "string") {
      return WsSender.withManager(obj.Manager.username, obj.Manager.user_ctx_hash);
    } else {
      throw new Error("Invalid object for WsSender");
    }
  }

  static fromJson(json) {
    const obj = JSON.parse(json);
    return this.fromObj(obj);
  }

  toJson() {
    return JSON.stringify(this.asObj());
  }

  equals(other) {
    if (!(other instanceof WsSender)) {
      throw new Error("compare only work in same type");
    }
    return this.toJson() === other.toJson();
  }

  is(other) {
    if (!(other instanceof WsSender)) {
      throw new Error("compare only work in same type");
    }
    return this.#value == other.#value;
  }
}

class WsDispatchType {
  static Broadcast = new WsDispatchType(0, null);
  static Server = new WsDispatchType(1, null);
  static Targets = new WsDispatchType(2, null);
  static BroadcastSameUser = new WsDispatchType(3, null);
  static withTargets = clients => {
    if (!Array.isArray(clients)) {
      throw new Error("users should be array");
    }
    clients.forEach(client => {
      if (!(client instanceof WsClient)) {
        throw new Error("should be client");
      }
    });
    return new WsDispatchType(3, clients);
  }
  #value
  #wsClients

  constructor(val, clients) {
    this.#value = val;
    if (clients == null) return;
    this.#wsClients = clients;
  }

  get wsClients() {
    return this.#wsClients;
  }

  set WsClients(clients) {
    if (clients && (Array.isArray(clients) && !(clients.at(0) instanceof WsClient))) {
      throw new Error("type is not true");
    }
    this.#wsClients = clients;
  }

  asObj() {
    let out_obj;
    switch (this.#value) {
      case 0:
        out_obj = "Broadcast";
        break;
      case 1:
        out_obj = "Server";
        break;
      case 2: 
        out_obj = { Targets: pclone(this.#wsClients) };
        break;
      case 3:
        out_obj = "BroadcastSameUser";
        break;
      default:
        throw new Error("unexpected");
    }
    return out_obj;
  }

  static fromObj(obj) {
    if (obj === "Broadcast") {
      return WsDispatchType.Broadcast;
    } else if (obj === "BroadcastSameUser") {
      return WsDispatchType.Broadcast;
    } else if (obj === "Server") {
      return WsDispatchType.Server;
    } else if (typeof obj === "object" && obj !== null && obj.Targets) {
      let clients = [];
      obj.Targets.forEach(client => {
        clients.push(new WsClient(client.username, client.user_ctx_hash));
      });
      return WsDispatchType.withTargets(clients);
    } else {
      throw new Error("Invalid object for WsDispatchType");
    }
  }

  static fromJson(json) {
    const obj = JSON.parse(json);
    return this.fromObj(obj);
  }

  toJson() {
    return JSON.stringify(this.asObj());
  }

  equals(other) {
    if (!(other instanceof WsDispatchType)) {
      throw new Error("compare only work in same type");
    }
    return this.toJson() === other.toJson();
  }

  is(other) {
    if (!(other instanceof WsDispatchType)) {
      throw new Error("compare only work in same type");
    }
    return this.#value == other.#value;
  }
}

class WsMessageClass {
  static Establish = new WsMessageClass(0, null);
  static Leave = new WsMessageClass(7, null);
  static FileSendable = new WsMessageClass(1, null);
  static withFileSendable = e => {
    return new WsMessageClass(1, e);
  };
  static Text = new WsMessageClass(2, null);
  static withText = text => {
    return new WsMessageClass(2, text);
  };
  static Errjson = new WsMessageClass(3, null);
  static withErrjson = msg => {
    return new WsMessageClass(3, msg)
  };
  static FileRequest = new WsMessageClass(4, null);
  static withFileRequest = e => {
    return new WsMessageClass(4, e);
  };
  static FileResponse = new WsMessageClass(5, null);
  static withFileResponse = e => {
    return new WsMessageClass(5, e);
  };
  static Notify = new WsMessageClass(6, null);
  static withNotify = text => {
    return new WsMessageClass(6, text);
  };
  #value
  #content

  constructor(val, content) {
    this.#value = val;
    this.#content = content;
  }

  get content() {
    return this.#content;
  }

  set content(c) {
    this.#content = c;
  }

  asObj() {
    let out_obj;
    switch (this.#value) {
      case 0:
        out_obj = "Establish";
        break;
      case 1:
        out_obj = { FileSendable: this.#content };
        break;
      case 2:
        out_obj = { Text: this.#content };
        break;
      case 3:
        out_obj = { Errjson: this.#content };
        break;
      case 4:
        out_obj = { FileRequest: this.#content };
        break;
      case 5:
        out_obj = { FileResponse: this.#content };
        break;
      case 6:
        out_obj = { Notify: this.#content };
        break;
      case 7:
        out_obj = "Leave";
        break;
    }
    return out_obj;
  }

  static fromJson(json) {
    const obj = JSON.parse(json);
    return this.fromObj(obj);
  }

  static fromObj(obj) {
    if (obj === "Establish") {
      return WsMessageClass.Establish;
    } else if (obj === "Leave") {
      return WsMessageClass.Leave;
    } else if (typeof obj === 'object' && obj !== null && obj.FileRequest) {
      return WsMessageClass.withFileRequest(obj.FileRequest);
    } else if (typeof obj === 'object' && obj !== null && obj.FileResponse) {
      return WsMessageClass.withFileResponse(obj.FileResponse);
    } else if (typeof obj === 'object' && obj !== null && obj.FileSendable) {
      return WsMessageClass.withFileSendable(obj.FileSendable);
    } else if (typeof obj === 'object' && obj !== null && obj.Text) {
      return WsMessageClass.withText(obj.Text);
    } else if (typeof obj === 'object' && obj !== null && obj.Notify) {
      return WsMessageClass.withNotify(obj.Notify);
    } else if (typeof obj === 'object' && obj !== null && obj.Errjson) {
      return WsMessageClass.withErrjson(obj.Errjson);
    } else {
      throw new Error("Invalid object for WsMessageClass");
    }
  }

  toJson() {
    return JSON.stringify(this.asObj());
  }

  equals(other) {
    if (!(other instanceof WsMessageClass)) {
      throw new Error("compare only work in same type");
    }
    return this.toJson() === other.toJson();
  }

  is(other) {
    if (!(other instanceof WsMessageClass)) {
      throw new Error("compare only work in same type");
    }
    return this.#value == other.#value;
  }
}

class WsMessage {
  #sender
  #msg
  #policy
  constructor(sender, ws_message_class, policy) {
    if (!(sender instanceof WsSender)) {
      throw new Error("inner error");
    }
    if (!(ws_message_class instanceof WsMessageClass)) {
      throw new Error("inner error");
    }
    if (!(policy instanceof WsDispatchType)) {
      throw new Error("inner error");
    }
    this.#sender = sender;
    this.#msg = ws_message_class;
    this.#policy = policy;
  }

  get sender() {
    return this.#sender;
  }

  get msg() {
    return this.#msg;
  }

  get policy() {
    return this.#policy;
  }

  set sender(sender) {
    if (!(sender instanceof WsSender)) {
      throw new Error("inner error");
    }
    this.#sender = sender;
  }

  set msg(msg) {
    if (!(msg instanceof WsMessageClass)) {
      throw new Error("inner error");
    }
    this.#msg = msg;
  }

  set policy(policy) {
    if (!(policy instanceof WsDispatchType)) {
      throw new Error("inner error");
    }
    this.#policy = policy;
  }

  asObj() {
    return {
      sender: this.#sender.asObj(),
      msg: this.#msg.asObj(),
      policy: this.#policy.asObj(),
    };
  }

  static fromJson(json) {
    const obj = JSON.parse(json);
    return this.fromObj(obj);
  }

  static fromObj(obj) {
    const sender = WsSender.fromObj(obj.sender);
    const msg = WsMessageClass.fromObj(obj.msg);
    const policy = WsDispatchType.fromObj(obj.policy);
    return new WsMessage(sender, msg, policy);
  }

  // to json string for send
  toJson() {
    return JSON.stringify(this.asObj());
  }

  equals(other) {
    if (!(other instanceof WsMessage)) {
      throw new Error("compare only work in same type");
    }
    return this.toJson() === other.toJson();
  }
}

function wssend(msg) {
  console.log("ws send: ", msg);
  data.ws.socket.send(msg);
}

function onNotify(ws_message) {
  if (!(ws_message instanceof WsMessage)) {
    throw new Error("type is not true");
  }
  console.log('receive notify: ', ws_message);

  let sender = "";
  let important = false;
  if (ws_message.sender.is(WsSender.Server)) {
    sender = "Pulsear";
    important = true;
  } else if (ws_message.sender.is(WsSender.User)) {
    sender = ws_message.sender.wsclient.username;
  } else { // Manager
    sender = ws_message.sender.wsclient.username;
    important = true;
  }
  let msg = ws_message.msg.content;

  let notification_container = document.getElementsByClassName('notification-container')[0];
  let newNode = document.createElement("div"); // anything is ok, it's only a tag
  if (important) {
    newNode.className = 'notification-important';
  } else {
    newNode.className = 'notification';
  }
  newNode.innerHTML = `${sender}: ${msg}`; 
  notification_container.appendChild(newNode);
}

function registerWs() {
  const { location } = window;

  const proto = location.protocol.startsWith('https') ? 'wss' : 'ws';
  const wsUri = `${proto}://${location.host}/ws`;

  data.ws.socket = new WebSocket(wsUri);

  data.ws.socket.onopen = evt => {
    console.log('Ws connected');
    let msg = new WsMessage(
      WsSender.withUser(data.userCtx.username, data.userCtx.user_ctx_hash),
      WsMessageClass.Establish,
      WsDispatchType.Server
    );
    wssend(msg.toJson());
  }

  data.ws.socket.onmessage = evt => {
    console.log('Ws received: ' + evt.data);
    let ws_message = WsMessage.fromJson(evt.data);
    if (ws_message.msg.is(WsMessageClass.Errjson)) {
      console.log('json decode error from server!');
    }
    if (ws_message.msg.is(WsMessageClass.Notify)) {
      onNotify(ws_message);
    }
    if (ws_message.msg.is(WsMessageClass.Establish)) {
      data.userCtx.user_ctx_hash = ws_message.policy.wsClients[0].user_ctx_hash;
    }
    if (ws_message.msg.is(WsMessageClass.Leave)) {
      data.ws.socket.close();
      data.userCtx.login = false
      data.userCtx.username = "";
      data.userCtx.token = "";
      data.localConfig.userToken = "";
      data.localConfig.username = "";
    }
    if (ws_message.msg.is(WsMessageClass.FileSendable) ||
        ws_message.msg.is(WsMessageClass.FileResponse)) {
      data.uploader.onWsMessage(ws_message);
    }
  }

  data.ws.socket.onclose = evt => {
    console.log('Ws disconnected');
    data.ws.socket = null;
  }

  data.ws.socket.onerror = evt => {
    console.log("Ws error: ", evt);
    data.ws.socket = null;
  }
}
