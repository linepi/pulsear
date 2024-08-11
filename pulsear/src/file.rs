use crate::*;

// all thing about a file and its transfer
// binary package:
//  bytes:   |   32      |  4        |  slice_size  |
//  meaning: |  file_hash| slice_idx | file content |
#[derive(serde::Deserialize, serde::Serialize, Clone)]
pub struct FileRequest {
  pub username: String,
  pub name: String,
  pub size: u64,
  pub slice_size: u64,
  pub last_modified_t: u64,
  pub file_hash: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub enum FileResponseStatus {
  Ok,
  Finish, // finish when the last slice index is Ok
  Resend,
  Fatalerr,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct FileResponse {
  pub name: String, // filename
  pub file_hash: String,
  pub slice_idx: (u64, u64),
  pub status: FileResponseStatus,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FileListElem {
  pub name: String,
  pub size: String,
  pub create_t: String,
  pub access_t: String,
  pub modify_t: String,
}

impl FileListElem {
  pub fn columns() -> Vec<String> {
    vec!["name", "size", "create_t", "access_t", "modify_t"]
        .iter_mut()
        .map(|str| str.to_string())
        .collect()
  }

  pub fn from_name_and_metadata(name: String, metadata: std::fs::Metadata) 
    -> Result<Self, Err> {
    let size_in_bytes = metadata.size();
    let size: String = |bytes| -> String {
      if bytes < 1024 {
        return format!("{}b", bytes);
      } else if bytes < 1024 * 1024 {
        return format!("{:.1}Kb", bytes as f64 / 1024.0);
      } else if bytes < 1024 * 1024 * 1024 {
        return format!("{:.3}Mb", bytes as f64 / 1024.0 / 1024.0);
      } else {
        return format!("{:.5}Gb", bytes as f64 / 1024.0 / 1024.0 / 1024.0);
      }
    } (size_in_bytes);
    let create_t = Time::from(metadata.created()?).as_fmt("%Y-%m-%d %H:%M:%S");
    let modify_t = Time::from(metadata.modified()?).as_fmt("%Y-%m-%d %H:%M:%S");
    let access_t = Time::from(metadata.accessed()?).as_fmt("%Y-%m-%d %H:%M:%S");
    Ok(Self {
      name,
      size,
      create_t,
      modify_t,
      access_t
    })
  }

  pub fn from(username: String, filename: String, size: u64) 
    -> Result<Self, Err> {
    let storage = std::path::PathBuf::from("inner/storage");
    let userfile = storage.join(&username).join(&filename);
    let file = std::fs::File::open(userfile)?;
    let mut file_elem = FileListElem::from_name_and_metadata(filename, file.metadata()?)?;
    let size: String = |bytes| -> String {
      if bytes < 1024 {
        return format!("{}b", bytes);
      } else if bytes < 1024 * 1024 {
        return format!("{:.1}Kb", bytes as f64 / 1024.0);
      } else if bytes < 1024 * 1024 * 1024 {
        return format!("{:.3}Mb", bytes as f64 / 1024.0 / 1024.0);
      } else {
        return format!("{:.5}Gb", bytes as f64 / 1024.0 / 1024.0 / 1024.0);
      }
    } (size);
    file_elem.size = size;
    Ok(file_elem)
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct FileList {
  files: Vec<FileListElem>
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FileListRequest {
  username: String,
  token: String
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FileElemRequest {
  pub name: String,
  pub username: String,
  pub token: String
}

pub fn do_get_file_list(param: web::Json<FileListRequest>, data: web::Data<Arc<Server>>) 
  -> Result<HttpResponse, Err> {
  let mut list = FileList {
    files: vec![]
  };
  let sqlhandler = SqlHandler::new(data.dbpool.clone());
  let user = match sqlhandler.get_user_by_name(&param.username)? {
    Some(u) => {
      assert_eq!(&u.username, &param.username);
      assert_eq!(&u.token, &param.token);
      u
    }
    None => {
      return Err(Box::from("user not exists"));
    }
  };

  let storage = std::path::PathBuf::from("inner/storage");
  let userfolder = storage.join(&user.username);
  if !userfolder.exists() {
    std::fs::create_dir_all(&userfolder)?;
  }
  let read_dir = std::fs::read_dir(userfolder)?;
  for path in read_dir {
    let entry = path?;
    list.files.push(FileListElem::from_name_and_metadata(
      entry.file_name().into_string().unwrap(), 
      entry.metadata()?
    )?);
  }
  Ok(HttpResponse::Ok().body(serde_json::to_string(&list).unwrap()))
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DownloadRequest {
  pub name: String,
  pub username: String,
  pub token: String
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct DeleteFileRequest {
  pub name: String,
  pub username: String,
  pub token: String
}

struct FileWorker {
  jobs: RwLock<HashMap<String, FileJob>>,
}

impl FileWorker {
  fn new() -> Self {
    Self {
      jobs: RwLock::new(HashMap::new()),
    }
  }

  fn add_job(&self, file_hash: String, job: FileJob) {
    self.jobs.write().unwrap().insert(file_hash, job);
  }

  fn work(&self, file_hash: String, index: u64, data: bytes::Bytes) {
    use std::os::unix::prelude::FileExt;
    let jobs = self.jobs.read().unwrap();
    let job = jobs.get(&file_hash).unwrap();
    match job.file.write_at(&data, job.request.slice_size*index) {
      Ok(sz) => {
        if sz == data.len() {
          job.on_slice_send(index);
        } else {
          log::info!("slice sended byte: {}, but need {}", sz, data.len());
          job.on_slice_not_send(index);
        }
      }
      Err(e) => {
        log::debug!("slice not send, err: {}", e);
        job.on_slice_not_send(index);
      }
    }
  }

  fn done(&self, file_hash: String) {
    let mut jobs = self.jobs.write().unwrap();
    let job = jobs.get(&file_hash).unwrap();
    job.timer.stop_timer();
    jobs.remove(&file_hash);
  }
}

struct FileJob {
  request: FileRequest,
  user_ctx: UserCtx,
  file: std::fs::File,
  timer: Timer
}

impl FileJob {
  fn new(req: FileRequest, user_ctx: UserCtx) -> Result<Self, Err> {
    let storage = std::path::PathBuf::from("inner/storage");
    let userfolder = storage.join(&req.username);
    if !userfolder.exists() {
      std::fs::create_dir_all(&userfolder)?;
    }
    let filepath = userfolder.join(&req.name);
    let f = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(filepath)?;
    let uctx = user_ctx.clone();
    let filehash = req.file_hash.clone();
    Ok(Self {
      request: req,
      file: f,
      user_ctx,
      timer: Timer::new(Duration::from_secs(10), move || {
        uctx.session.as_ref().unwrap().do_send(WsMessage {
          sender: WsSender::Server,
          msg: WsMessageClass::PleaseSend(filehash.clone()),
          policy: WsDispatchType::Targets(vec![WsClient::new(&uctx)])
        })
      })
    })
  }

  fn on_slice_not_send(&self, index: u64) {
    self.timer.reset_timer();
    let policy = WsDispatchType::Targets(vec![WsClient::new(&self.user_ctx)]);
    // the last index
    let status = FileResponseStatus::Resend;
    self.user_ctx.session.as_ref().unwrap().do_send(WsMessage {
      sender: WsSender::Server,
      msg: WsMessageClass::FileResponse(FileResponse {
        name: self.request.name.clone(),
        file_hash: self.request.file_hash.clone(),
        slice_idx: (index, index+1),
        status
      }),
      policy
    })
  }

  fn on_slice_send(&self, index: u64) {
    self.timer.reset_timer();
    let status: FileResponseStatus;
    let policy = WsDispatchType::BroadcastSameUser;
    // the last index
    status = FileResponseStatus::Ok;
    self.user_ctx.session.as_ref().unwrap().do_send(WsMessage {
      sender: WsSender::Server,
      msg: WsMessageClass::FileResponse(FileResponse {
        name: self.request.name.clone(),
        file_hash: self.request.file_hash.clone(),
        slice_idx: (index, index+1),
        status
      }),
      policy
    });
  }
}

pub struct FileHandler {
  worker_num: i32,
  workers: Vec<FileWorker>,
  // dispatch file to worker
  worker_dispatch: RwLock<HashMap<String, usize>>,
  // download codes: map hash to username, filename
  codes: RwLock<HashMap<String, (String, String)>>,
}

impl FileHandler {
  pub fn new(worker_num: i32) -> Self {
    let mut me = Self { 
      worker_num,
      workers: vec![],
      worker_dispatch: RwLock::new(HashMap::new()),
      codes: RwLock::new(HashMap::new()),
    };

    for _ in 0..worker_num {
      me.workers.push(FileWorker::new());
    }
    me
  }

  pub fn add(&self, req: FileRequest, user_ctx: UserCtx) -> bool {
    let job = match FileJob::new(req.clone(), user_ctx) {
      Ok(j) => j,
      Err(_) => {
        log::error!("add file error");
        return false;
      }
    };

    let worker_id = (Time::now().milli() % self.worker_num as u64) as usize;
    log::info!("map file{} to worker_id {}", req.file_hash, worker_id);
    assert!(self.worker_dispatch.write().unwrap().insert(req.file_hash.clone(), worker_id).is_none());
    self.workers[worker_id].add_job(req.file_hash, job);
    true
  }

  pub fn done(&self, file_hash: String) {
    let worker = &self.workers[*self.worker_dispatch.read().unwrap().get(&file_hash).unwrap()];
    worker.done(file_hash);
  }

  pub fn send(&self, bytes: bytes::Bytes) {
    let hashstr: String = bytes.slice(0..32).iter().map(|b| {
      format!("{:02x}", b).to_string()
    }).collect();
    let index: u64 = bytes.slice(32..36).get_u32_le() as u64;
    log::warn!("SEND {} {}", hashstr, index);
    let worker = &self.workers[*self.worker_dispatch.read().unwrap().get(&hashstr).unwrap()];
    worker.work(hashstr, index, bytes.slice(36..));
  }

  pub fn delete_file(&self, req: DeleteFileRequest) -> Result<(), Err> {
    Ok(std::fs::remove_file(format!("inner/storage/{}/{}", req.username, req.name))?)
  }

  pub fn gen_download_code(&self, req: DownloadRequest) -> String {
    let code = sha256::digest(serde_json::to_string(&req).unwrap() + format!("{}", Time::now()).as_str());
    let mut codes = self.codes.write().unwrap();
    assert!(codes.insert(code.clone(), (req.username, req.name)).is_none());
    code
  }

  pub fn from_download_code(&self, code: &String) -> Option<(String, String)> {
    let codes = self.codes.read().unwrap();
    match codes.get(code) {
      Some(p) => Some((p.0.clone(), p.1.clone())),
      None => None
    }
  }

  pub fn get_user_used_storage(&self, username: &String) -> Result<u64, Err> {
    let storage = std::path::PathBuf::from("inner/storage");
    let userfolder = storage.join(username);
    if !userfolder.exists() {
      return Ok(0);
    }
    let read_dir = std::fs::read_dir(userfolder)?;
    let mut ret = 0;
    for path in read_dir {
      let entry = path?;
      ret += entry.metadata()?.size();
    }
    Ok(ret)
  }
}
