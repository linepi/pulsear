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
        this.uploadAll(this.#files[hashval]);
      }
    }
    if (ws_message.msg.is(WsMessageClass.FileResponse)) {
      let resp = ws_message.msg.content;
      const file = this.#files[resp.file_hash];
      let uploadStatus = {
        nr_slice_all: parseInt((file.req.size - 1) / file.req.slice_size + 1),
        nr_slice_ok: file.nr_success_resp
      }
      if (resp.status === "Finish") {
        onFileUploaded(resp.name);
        this.#files[resp.file_hash].nr_success_resp++;
        uploadStatus.nr_slice_ok++;
        delete(this.#files[resp.file_hash]);
        updateUploadStatus(resp.name, uploadStatus);
      } else if (resp.status === "Ok") {
        this.#files[resp.file_hash].nr_success_resp++;
        uploadStatus.nr_slice_ok++;
        updateUploadStatus(resp.name, uploadStatus);
      } else if (resp.status === "Resend") {
        let start = file.req.slice_size*resp.slice_idx;
        this.uploadSlice(file.f, file.req.file_hash, resp.slice_idx, 
            start, 
            Math.min(file.req.slice_size, file.req.size - start));
      } else if (resp.status === "Fatalerr") {
        notify(true, `upload ${resp.name} error`);
        delete(this.#files[resp.file_hash]);
      }
    }
  }

  // use wssend to send all slice to server async
  // { f: file, req: request, nr_success_resp: 0 }
  uploadAll(file) {
    let slice_size = file.req.slice_size;
    let size = file.req.size;
    let i = 0;
    let n = parseInt((size - 1) / slice_size + 1);

    while (i < n) {
      let sendsize = slice_size;
      if (i == n - 1) {
        sendsize = size - i*slice_size;
      }
      this.uploadSlice(file.f, file.req.file_hash, i, i*slice_size, i*slice_size + sendsize);
      i++;
    }
  }

  uploadSlice(file, hashval, i, start, end) {
    const hashvalBlob = new Uint8Array(hashval.match(/[\da-f]{2}/gi).map(byte => parseInt(byte, 16)));
    const view = new DataView(new ArrayBuffer(4));
    view.setUint32(0, i, true); // true express little-endian
    const sliceIndexBlob = new Blob([new Uint8Array(view.buffer)]);
    const fileSliceBlob = file.slice(start, end); 
    let blobToSend = new Blob([hashvalBlob, sliceIndexBlob, fileSliceBlob]);
    wssend(blobToSend)
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
    console.log('upload ', file);
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
    this.#files[hashval] = { f: file, req: request, nr_success_resp: 0 };
  }
}