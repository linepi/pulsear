function onPopLoginWindow(el) {
  el.style.display = "flex";
  el.focus();
}

function loginInput(evt) {
  let input = evt.target;
  if (input.type === 'text') {
    data.loginCtx.usernameInput = input.value;
  } else if (input.type === 'password') {
    data.loginCtx.passwordInput = input.value;
  } else {
    console.error('unexpected');
  }
}

function doLogin(isInit) {
  let username;
  let choice = {};
  if (isInit) {
    username = data.localConfig.username;
    choice.Token = data.localConfig.userToken;
  } else {
    username = data.loginCtx.usernameInput;
    choice.Password = data.loginCtx.passwordInput;
    if (choice.Password.length < 4 ||
      choice.Password.length > 16) {
      data.loginCtx.alertMessage = "password length not valid";
      console.log(data.loginCtx.alertMessage, choice.Password.length);
      return;
    }
  }
  if (username.length < 4 ||
    username.length > 16) {
    data.loginCtx.alertMessage = "username length not valid";
    console.log(data.loginCtx.alertMessage)
    return;
  }

  let loginRequest = {
    basic_info: {
      time_stamp: Date.now()
    },
    login_info: {
      username: username,
      choice: choice
    }
  }
  let ret = true;
  console.log("try login with request ", loginRequest);
  fetch(data.api.login, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json; charset=UTF-8'
    },
    body: JSON.stringify(loginRequest)
  }).then(response => {
    if (!response.ok) {
      console.error("login bad response: ", response);
      data.loginCtx.alertMessage = "server response: " + response.text();
      throw new Error(data.loginCtx.alertMessage);
    }
    return response.json();
  }).then(json => {
    console.log('get login response: ', json);
    onLogin(json, isInit);
  }).catch(error => {
    data.loginCtx.alertMessage = error;
    console.error(data.loginCtx.alertMessage);
    ret = false;
  })
  return ret;
}

function doLogout() {
  let logoutRequest = {
    basic_info: {
      time_stamp: Date.now()
    },
    username: data.userCtx.username,
    token: data.userCtx.token,
  };
  console.log("try logout with request ", logoutRequest);
  fetch(data.api.logout, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json; charset=UTF-8'
    },
    body: JSON.stringify(logoutRequest)
  }).then(response => {
    if (!response.ok) {
      console.error("logout bad response:", response);
      throw new Error(data.loginCtx.alertMessage);
    }
    return response.json();
  }).then(json => {
    console.log('get logout response: ', json);
    onLogout(json);
  }).catch(error => {
    console.error("logout error ", error);
  })
}

function onResponseCode(code) {
  // if server responsed code is enum::Ok, the code is "Ok" which is a string
  // else enum::Err, then the code is { "Err": errmsg } which is an object
  if (typeof code === "object") {
    throw new Error(code.Err);
  }
}

function onLogin(response, isInit) {
  onResponseCode(response.code);
  // login success
  data.localConfig.userToken = response.token;
  data.userCtx.login = true
  data.userCtx.token = response.token;
  data.localConfig.userconfig = observe(response.config);
  console.log('Get userconfig on login: ', data.localConfig.userconfig);
  if (isInit) {
    data.userCtx.username = data.localConfig.username;
  } else {
    data.localConfig.username = data.loginCtx.usernameInput;
    data.userCtx.username = data.loginCtx.usernameInput;
  }
  data.loginCtx.alertMessage = "";
  data.loginWindow = false

  setTimeout(async () => {
    while (true) {
      try {
        eval('registerWsMain');
        eval('registerWsWorker');
        eval('Uploader');
        registerWsMain(false);
        registerWsWorker();
        data.uploader = new Uploader();
        break;
      } catch {
        await new Promise(r => setTimeout(r, 2000));
      }
    }
  }, 100);
}

function onLogout(response) {
  onResponseCode(response.code);
  let msg = new WsMessage(
    WsSender.withUser(data.userCtx.username, data.userCtx.user_ctx_hash),
    WsMessageClass.Leave,
    WsDispatchType.Server
  );
  wssend(msg.toJson());
}

function onUserProfile() {

}

function onUserSettings() {

}
