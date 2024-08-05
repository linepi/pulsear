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

  toJson() {
    return JSON.stringify(this.asObj());
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
  #name;

  constructor(val, name) {
    this.#value = val;
    this.#name = name;
  }

  asObj() {
    let out_obj;
    switch (this.#value) {
      case 0:
        out_obj = "Server";
        break;
      case 1:
        out_obj = { User: pclone(this.#name) };
        break;
      case 2:
        out_obj = { Manager: pclone(this.#name) };
        break;
      default:
        throw new Error("unexpected");
    }
    return out_obj;
  }

  toJson() {
    return JSON.stringify(this.asObj());
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
    new WsDispatchType(3, username_list)
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

  toJson() {
    return JSON.stringify(this.asObj());
  }
}

class WsMessageClass {
  static Establish = new WsMessageClass(0, null);
  static File = file => {
    new WsMessageClass(1, file);
  };
  static Text = text => {
    new WsMessageClass(2, text);
  };
  static Errjson = msg => {
    new WsDispatchType(3, msg)
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

  toJson() {
    return JSON.stringify(this.asObj());
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

  // to json string for send
  toJson() {
    return JSON.stringify(this.asObj());
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
