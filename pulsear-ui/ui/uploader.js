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
      let file_sendable_resp = ws_message.msg.content;
      let hashval = file_sendable_resp.hashval;
      let user_ctx_hash = file_sendable_resp.user_ctx_hash;
      // other client of this user should create a new one
      if (!this.#files[hashval]) {
        this.#files[hashval] = {};
      }
      let file = this.#files[hashval];
      if (data.userCtx.user_ctx_hash === user_ctx_hash) {
        file.isUploader = true;
      } else {
        file.isUploader = false;
      }
      if (typeof file_sendable_resp.file_elem === 'object') {
        let file_elem = file_sendable_resp.file_elem;
        let req = file_sendable_resp.req;

        let tbody = document.querySelector('tbody');
        let tr = createFileRowElem(file_elem);
        let name_td = tr.childNodes[0];
        let overlay = document.createElement('div');
        overlay.className = 'td-overlay';
        overlay.style.opacity = 0.85;
        name_td.style.position = 'relative';
        name_td.appendChild(overlay);
        tbody.appendChild(tr);

        file.tr = tr;
        file.name_td = name_td;
        file.name_overlay = overlay;
        file.req = req;
        file.upload = {
          nr_slice_all: parseInt((req.size - 1) / req.slice_size + 1),
          nr_slice_ok: 0
        };
        if (file.isUploader) {
          giveWorkerMsg(0, {
            req: file.req,
            f: file.f,
          });
        }
        this.updateUploadStatus(file.name_overlay, file.upload);
        this.notifyWrapper(false, "upload file " + file_elem.name, file.isUploader);
      } else {
        this.notifyWrapper(false, "sorry, you cannot send ", file.req.name,
          file.isUploader);
        if (file.isUploader) {
          delete (this.#files[resp.hashval]);
        }
      }
    }
    if (ws_message.msg.is(WsMessageClass.FileResponse)) {
      let resp = ws_message.msg.content;
      const file = this.#files[resp.file_hash];
      if (file.upload == null) {
        console.log('upload is null: ', file);
      }
      if (resp.status === "Finish" || resp.status === "Ok") {
        file.upload.nr_slice_ok++;
        this.updateUploadStatus(file.name_overlay, file.upload);
        if (resp.status === "Finish") {
          this.onFileUploaded(file);
          delete (this.#files[resp.file_hash]);
        }
      } else if (resp.status === "Resend" && this.#files[resp.file_hash].isUploader) {
        // let start = file.req.slice_size*resp.slice_idx;
        // this.uploadSlice(file.f, file.req.file_hash, resp.slice_idx, 
        //     start, 
        //     Math.min(file.req.slice_size, file.req.size - start));
      } else if (resp.status === "Fatalerr") {
        this.notifyWrapper(true, `upload ${resp.name} error`, this.#files[resp.file_hash].isUploader);
        delete (this.#files[resp.file_hash]);
      }
    }
  }

  // status {
  //   nr_slice_all: , 
  //   nr_slice_ok: ,
  // }
  updateUploadStatus(overlay, status) {
    let percent = status.nr_slice_ok / status.nr_slice_all; // 这是一个0到1之间的值
    let upload_percent = percent * 100;
    if (percent != 1) {
      overlay.style.opacity = (0.85 - 0.5 * percent).toFixed(2); // 从20%的透明度开始到100%的不透明
    } else {
      overlay.style.opacity = 0;
    }
    overlay.textContent = `${upload_percent.toFixed(2)}% uploaded`;
  }


  onFileUploaded(file) {
    let suffix = "";
    if (!file.isUploader) {
      suffix += " in other place"
    }

    let filename = file.req.name;
    notify(false, `upload ${filename} success ${suffix}`);
  }

  notifyWrapper(important, msg, isUploader) {
    notify(important, `${!isUploader ? "In other place: " : ""}${msg}`)
  }

  // a file hash contain username, filename, filesize, filelastmodify time
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

  bytesToHumanReadbleString(bytes) {
    if (bytes < 1024) {
      return `${bytes}b`;
    } else if (bytes < 1024 * 1024) {
      return `${(bytes / 1024.0).toFixed(1)}Kb`;
    } else if (bytes < 1024 * 1024 * 1024) {
      return `${(bytes / 1024.0 / 1024.0).toFixed(3)}Mb`;
    } else {
      return `${(bytes / 1024.0 / 1024.0 / 1024.0).toFixed(5)}Gb`;
    }
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
    this.#files[hashval] = {
      f: file,
      req: request,
      tr: null,
      name_td: null,
      name_overlay: null
    };
  }
}