function defaultLocalConfig() {
 return {
    userconfig: {
      theme: "dark",
      web_worker_num: 4,
      filelist_config: {
        "/": {
          order_by: "name",
          order_asc: true,
          columns: ["name", "size", "create_t", "access_t", "modify_t"]
        }
      },
    },
    notify: true,
    userToken: "",
    username: "",
  }
}

function bytesToHumanReadbleString(bytes) {
  if (bytes < 1024) {
    return `${bytes}b`;
  } else if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)}Kb`;
  } else if (bytes < 1024 * 1024 * 1024) {
    return `${(bytes / 1024 / 1024).toFixed(2)}Mb`;
  } else if (bytes < 1024 * 1024 * 1024 * 1024) {
    return `${(bytes / 1024 / 1024 / 1024).toFixed(3)}Gb`;
  } else {
    return `${(bytes / 1024 / 1024 / 1024 / 1024).toFixed(4)}Tb`;
  }
}

// -------------- init -----------------
let root = document;
let prefix_ = window.location.origin + '/';

let rawData = {
  prefix: prefix_,
  resources_prefix: prefix_ + 'resources/',
  tab: 'home',
  webTitle: 'Pulsear',
  pageTitle: 'Pulsear',
  loginWindow: false,
  userCtx: {
    username: "",
    token: "",
    login: false,
    userMenu: false,
    user_ctx_hash: ""
  },
  loginCtx: {
    usernameInput: "",
    passwordInput: "",
    alertMessage: ""
  },
  api: {
    login: prefix_ + "login",
    logout: prefix_ + "logout",
    getfile: prefix_ + "files",
    getfileelem: prefix_ + "file",
    getdownloadurl: prefix_ + "get_download_url",
    downloadbyurl: prefix_ + "download",
  },
  localConfig: defaultLocalConfig(),
  ws: {
    established: false,
    socket: null,
    workers: [],
  },
  files: {
    uploadText: "upload",
    downloadText: "download",
  },
  uploader: null,
  dashboard: {
    info: {
      online_user: 0,
      online_client: 0,
      user_used_storage: 0,
      user_max_storage: 0,
    },
    cards: [
      {
        header: "Storage",
      },
      {
        header: "Users",
      },
      {
        header: "Clients",
      },
      {
        header: "Other",
      }
    ]
  }
}

for (let i = 0; i < rawData.localConfig.userconfig.web_worker_num; i++) {
  rawData.ws.workers.push({
    id: i,
    worker: null
  });
}

let directives = {
  'v-text': (el, expr) => {
    el.innerText = eval(expr);
  },
  'v-var': (el, expr) => {
    let raw;
    if (!el.hasAttribute('raw-text')) {
      el.setAttribute('raw-text', el.innerText);
      raw = el.innerText;
    } else {
      raw = el.getAttribute('raw-text') 
    }
    let matchs = raw.match(/{{(.*?)}}/g);
    if (matchs) {
      matchs.forEach(match => {
        let varName = match.slice(2, -2).trim();  
        let val = eval(varName);
        raw = raw.replace(match, val);
      });
    }
    el.innerText = raw;
  },
  'v-show': (el, expr) => {
    el.style.display = eval(expr) ? 'block' : 'none';
  },
  'v-show-bind': (el, func) => {
    if (el.style.display !== 'none') window[func](el);
  },
  'v-src': (el, expr) => {
    el.src = eval(expr);
    el.removeAttribute('v-src');
  },
  'v-href': (el, expr) => {
    el.href = eval(expr);
    el.removeAttribute('v-href');
  },
  'v-if': (el, value) => {
    let condition_dom_list = []
    while (el) {
      el.style.display = 'none';
      let expression = null;
      ['v-if', 'v-else-if', 'v-else'].forEach(conname => {
        if (el.hasAttribute(conname)) {
          expression = el.getAttribute(conname);
        }
      })
      condition_dom_list.push({ dom: el, expr: expression })
      el = el.nextElementSibling;
    }
    for (i in condition_dom_list) {
      let dom = condition_dom_list[i].dom;
      let expr = condition_dom_list[i].expr;
      if (i == condition_dom_list.length - 1 || eval(expr)) {
        dom.style.display = 'block';
        break;
      }
    }
  }
}


let data = {};

function jsSetValue(varString, valueString) {
  const keys = varString.split('.');
  let obj = window;
  for (let i = 0; i < keys.length - 1; i++) {
    console.log(obj, keys[i]);
    obj = obj[keys[i]];
  }
  obj[keys[keys.length - 1]] = valueString;
}

function observe(data) {
  if (typeof data !== 'object' || data === null) {
    return data;
  }

  const proxy = new Proxy(data, {
    set(target, key, value) {
      const oldValue = target[key];
      if (oldValue !== value) {
        target[key] = value; 
        refreshDom();
      }
    }
  });

  for (const key in data) {
    if (data.hasOwnProperty(key)) {
      data[key] = observe(data[key]);
    }
  }

  return proxy;
}

function doRefreshDom(el) {
  Array.from(el.attributes).forEach(attribute => {
    if (!Object.keys(directives).includes(attribute.name)) return;
      directives[attribute.name](el, attribute.value);
    }
  )
}

function doRegisterListeners(el) {
  Array.from(el.attributes).forEach(attribute => {
    let keytype = null;
    let event;
    if ((attribute.name.includes("keyup") || attribute.name.includes("keydown")) &&
      attribute.name.includes(".")) {
      keytype = attribute.name.split('.')[1];
      event = attribute.name.split('.')[0].replace('@', '');
    } else {
      event = attribute.name.replace('@', '');
    }
    let expression = attribute.value;
    el.addEventListener(event, evt => {
      if (keytype == null || evt.key.toLowerCase() === keytype) {
        if (typeof eval(expression) === 'function') {
          window[expression](evt);
        }
      }
    })
  })
}

function updateLocalConfig() {
  localStorage.setItem('pulsearLocalConfig', JSON.stringify(data.localConfig));
}

function loadLocalConfig(rawData) {
  if (localStorage.getItem('pulsearLocalConfig') != null) {
    let localConfig = JSON.parse(localStorage.getItem('pulsearLocalConfig'));
    try {
      localConfig.userconfig.filelist_config;
      rawData.localConfig = localConfig;
    } catch {
      rawData.localConfig = defaultLocalConfig();
    }
    console.log("localConfig init as: ", rawData.localConfig);
  }
}

function refreshDom() {
  console.log('refresh dom');
  document.documentElement.setAttribute('data-theme', data.localConfig.userconfig.theme);
  updateLocalConfig();
  walkDom(document.documentElement, doRefreshDom);
}

function walkDom(el, callback) {
  callback(el);
  el = el.firstElementChild;
  while (el) {
    walkDom(el, callback);
    el = el.nextElementSibling;
  }
}

function specialLisenter() {
  document.addEventListener('click', evt => {
    var userMenu = document.querySelector('.user-menu');
    var userMenuWrapper = document.querySelector('.user-menu-wrapper');
    if (!userMenuWrapper.contains(event.target) && !userMenu.contains(event.target)) {
      data.userCtx.userMenu = false;
    }
  });

  var lastScrollTop = 0;
  window.addEventListener("scroll", function () {
    var currentScroll = window.pageYOffset || document.documentElement.scrollTop;
    if (currentScroll > lastScrollTop) {
      document.querySelector("header").classList.add("header-transparent");
    } else {
      document.querySelector("header").classList.remove("header-transparent");
    }
    lastScrollTop = currentScroll <= 0 ? 0 : currentScroll;
  }, false);
}

async function getBody() {
  try {
    const response = await fetch(rawData.resources_prefix + 'body.html');
    if (!response.ok) {
      throw new Error('Network response was not ok.');
    }
    document.body.innerHTML = await response.text();
    walkDom(document.documentElement, doRegisterListeners)
    specialLisenter();
    refreshDom();
    if (data.localConfig.userToken && data.localConfig.username) {
      if (!doLogin(true)) {
        data.userCtx.login = false;
        data.userCtx.username = "";
        data.userCtx.token = "";
      }
    }
  } catch (error) {
    console.error('Fetch error:', error);
  }
}

loadLocalConfig(rawData);
data = observe(rawData);
