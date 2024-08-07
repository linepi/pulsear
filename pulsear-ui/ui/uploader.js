class Uploader {
  // #websocketNum
  // #workerNum

  // constructor(websocketNum, workerNum) {
  //   this.#websocketNum = websocketNum;
  //   this.#workerNum = workerNum;
  // }
  #files
  #sliceSize = 40960

  constructor() {
    this.#files = {};
  }

  onWsMessage(ws_message) {
    if (ws_message.msg.is(WsMessageClass.FileSendable)) {
      let hashval = ws_message.msg.content[0];
      let can = ws_message.msg.content[1];
      if (can) {
        this.uploadImpl(this.#files[hashval].f, hashval);
      }
    }
    if (ws_message.msg.is(WsMessageClass.FileResponse)) {
      let resp = ws_message.msg.content;
      if (resp.status == "Finish") {
        onFileUploaded(resp.name);
        delete(this.#files[resp.file_hash]);
      }
    }
  }

  // hashval: String
  uploadImpl(file, hashval) {
    let i = 0;
    let n = (file.size-1) / this.#sliceSize + 1;

    while (i < n) {
      let sendsize = this.#sliceSize;
      if (i == n - 1) {
        sendsize = file.size - i*this.#sliceSize;
      }
      const hashvalBlob = new Uint8Array(hashval.match(/[\da-f]{2}/gi).map(byte => parseInt(byte, 16)));
      const view = new DataView(new ArrayBuffer(4));
      view.setUint32(0, i, true); // true express little-endian
      const sliceIndexBlob = new Blob([new Uint8Array(view.buffer)]);
      const fileSliceBlob = file.slice(i*this.#sliceSize, i*this.#sliceSize + sendsize); 
      let blobToSend = new Blob([hashvalBlob, sliceIndexBlob, fileSliceBlob]);
      console.log(blobToSend);
      wssend(blobToSend)
      i++;
    }
  }

  hash(file) {
    let combinedStr = file.name + data.userCtx.username + 
        file.size.toString() + file.lastModified.toString();
    let hash = 0;
    for (let i = 0; i < combinedStr.length; i++) {
      hash = (hash << 5) - hash + combinedStr.charCodeAt(i);
      hash |= 0; 
    }
    const hashBytes = new Uint8Array(4); 
    for (let i = 0; i < 4; i++) {
      hashBytes[i] = (hash >> (i * 8)) & 0xff;
    }
    const result = new Uint8Array(32);
    for (let i = 0; i < 32; i++) {
      result[i] = hashBytes[i % 4]; 
    }
    return Array.from(result).map(b => b.toString(16).padStart(2, '0')).join('');
  }

  upload(file) {
    let hashval = this.hash(file);
    let request = {
      username: data.userCtx.username,
      name: file.name,
      size: file.size,
      slice_size: this.#sliceSize,
      last_modified_t: file.lastModified,
      file_hash: this.hash(file),
    };
    let msg = new WsMessage(
      WsSender.withUser(data.userCtx.username, data.userCtx.user_ctx_hash),
      WsMessageClass.withFileRequest(request),
      WsDispatchType.Server
    );
    wssend(msg.toJson());
    this.#files[hashval] = { f: file, req: request };
  }
}