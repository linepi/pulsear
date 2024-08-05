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
  #name
  constructor(name) {
    this.#name = name;
  }

  asObj() {
    return { username: pclone(this.#name) }
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
}

class WsSender {
  static Server = new WsSender(0, null);
  static User(name) {
    // see WsClient
    return new WsSender(1, new WsClient(name).asObj());
  }
  static Manager(name) {
    return new WsSender(2, new WsClient(name).asObj());
  }
  #value;
  #wsclient;

  constructor(val, wsclient) {
    this.#value = val;
    this.#wsclient = wsclient;
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
      return WsSender.User(obj.User.username);
    } else if (obj.Manager && typeof obj.Manager.username === "string") {
      return WsSender.Manager(obj.Manager.username);
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
}

class WsDispatchType {
  static Unknown = new WsDispatchType(0, null);
  static Broadcast = new WsDispatchType(1, null);
  static Server = new WsDispatchType(2, null);
  static Users = username_list => {
    if (!Array.isArray(username_list)) {
      throw new Error("users should be array");
    }
    return new WsDispatchType(3, username_list);
  }
  #value
  #ws_client_list

  constructor(val, list) {
    this.#value = val;
    if (list == null) return;
    this.#ws_client_list = [];
    list.forEach(name => {
      if (typeof name !== "string") {
        throw new Error("name should be string");
      }
      this.#ws_client_list.push(new WsClient(name).asObj())
    });
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
        out_obj = { Users: pclone(this.#ws_client_list) };
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
      return WsDispatchType.Users(username_list);
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
}

class WsMessageClass {
  static Establish = new WsMessageClass(0, null);
  static File = file => {
    return new WsMessageClass(1, file);
  };
  static Text = text => {
    return new WsMessageClass(2, text);
  };
  static Errjson = msg => {
    return new WsDispatchType(3, msg)
  };
  #value
  #content

  constructor(val, content) {
    this.#value = val;
    this.#content = content;
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
      return WsMessageClass.File(obj.File);
    } else if (typeof obj === 'object' && obj !== null && obj.Text) {
      return WsMessageClass.Text(obj.Text);
    } else if (typeof obj === 'object' && obj !== null && obj.Errjson) {
      return WsMessageClass.Errjson(obj.Errjson);
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

function registerWs() {
  const { location } = window;

  const proto = location.protocol.startsWith('https') ? 'wss' : 'ws';
  const wsUri = `${proto}://${location.host}/ws`;

  data.ws.socket = new WebSocket(wsUri);

  data.ws.socket.onopen = evt => {
    console.log('Ws connected');
    let msg = new WsMessage(
      WsSender.User(data.userCtx.username),
      WsMessageClass.Establish,
      WsDispatchType.Server
    );
    wssend(msg.toJson());
  }

  data.ws.socket.onmessage = evt => {
    console.log('Ws received: ' + evt.data);
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
