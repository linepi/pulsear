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
  constructor(name) {
    this.#username = name;
  }

  asObj() {
    return { username: pclone(this.#username) }
  }

  static fromObj(obj) {
    return new WsClient(obj.username);
  }

  static fromJson(json) {
    const obj = JSON.parse(json);
    return new WsClient(obj.username);
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
}

class WsSender {
  static Server = new WsSender(0, null);
  static User = new WsSender(1, null);
  static Manager = new WsSender(2, null);
  static withUser(name) {
    // see WsClient
    return new WsSender(1, new WsClient(name).asObj());
  }
  static withManager(name) {
    return new WsSender(2, new WsClient(name).asObj());
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
    } else if (obj.User && typeof obj.User.username === "string") {
      return WsSender.withUser(obj.User.username);
    } else if (obj.Manager && typeof obj.Manager.username === "string") {
      return WsSender.withManager(obj.Manager.username);
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
  static Unknown = new WsDispatchType(0, null);
  static Broadcast = new WsDispatchType(1, null);
  static Server = new WsDispatchType(2, null);
  static Users = new WsDispatchType(3, null);
  static withUsers = username_list => {
    if (!Array.isArray(username_list)) {
      throw new Error("users should be array");
    }
    return new WsDispatchType(3, username_list);
  }
  #value
  #wsClients

  constructor(val, list) {
    this.#value = val;
    if (list == null) return;
    this.#wsClients = [];
    list.forEach(name => {
      if (typeof name !== "string") {
        throw new Error("name should be string");
      }
      this.#wsClients.push(new WsClient(name).asObj())
    });
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
        out_obj = "Unknown";
        break;
      case 1:
        out_obj = "Broadcast";
        break;
      case 2:
        out_obj = "Server";
        break;
      case 3: 
        out_obj = { Users: pclone(this.#wsClients) };
        break;
      default:
        throw new Error("unexpected");
    }
    return out_obj;
  }

  static fromObj(obj) {
    if (obj === "Unknown") {
      return WsDispatchType.Unknown;
    } else if (obj === "Broadcast") {
      return WsDispatchType.Broadcast;
    } else if (obj === "Server") {
      return WsDispatchType.Server;
    } else if (typeof obj === "object" && obj !== null && obj.Users) {
      let username_list = [];
      obj.Users.forEach(ws_client => {
        username_list.push(ws_client.username);
      });
      return WsDispatchType.withUsers(username_list);
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
  static File = new WsMessageClass(1, null);
  static Text = new WsMessageClass(2, null);
  static Errjson = new WsMessageClass(3, null);
  static withFile = file => {
    return new WsMessageClass(1, file);
  };
  static withText = text => {
    return new WsMessageClass(2, text);
  };
  static withErrjson = msg => {
    return new WsDispatchType(3, msg)
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
        out_obj = { File: pclone(this.#content) };
        break;
      case 2:
        out_obj = { Text: pclone(this.#content) };
        break;
      case 3:
        out_obj = { Errjson: pclone(this.#content) };
        break;
      default:
        throw new Error("unexpected");
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
    } else if (typeof obj === 'object' && obj !== null && obj.File) {
      return WsMessageClass.withFile(obj.File);
    } else if (typeof obj === 'object' && obj !== null && obj.Text) {
      return WsMessageClass.withText(obj.Text);
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

function onBroadCast(ws_message) {
  if (!(ws_message instanceof WsMessage)) {
    throw new Error("type is not true");
  }
  console.log('receive broadcast: ', ws_message);

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
      WsSender.withUser(data.userCtx.username),
      WsMessageClass.Establish,
      WsDispatchType.Server
    );
    wssend(msg.toJson());
  }

  data.ws.socket.onmessage = evt => {
    console.log('Ws received: ' + evt.data);
    let ws_message = WsMessage.fromJson(evt.data);
    if (ws_message.policy.equals(WsDispatchType.Broadcast)) {
      onBroadCast(ws_message);
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
