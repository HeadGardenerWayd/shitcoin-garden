import {SigningCosmWasmClient} from "@cosmjs/cosmwasm-stargate";
import {calculateFee, GasPrice}  from "@cosmjs/stargate";
import {coin} from "@cosmjs/proto-signing";

const SHITCOIN_GARDEN = window.SHITCOIN_GARDEN;
const PRESALE_DENOM = window.PRESALE_DENOM;
const gasPrice = GasPrice.fromString("0.02untrn");

document.addEventListener('alpine:init', () => {
  Alpine.store('wallet', {
    installed: false,
    connected: false,
    wallet: null,
    presaleGetModifier: null,

    async init() {
      this.installed = window.keplr != undefined;
      this.connected = false;
      this.wallet = null;
      this.presaleGetModifier = null;

      const onKeyStoreChange = async () => {
        if (this.connected) {
          const accounts = await this.wallet.offlineSigner.getAccounts();
          const account = accounts[0];
          this.wallet.account = account;
          htmx.trigger(window, "address-change");
          htmx.trigger("#presales", "reload-all")
        }
      }

      window.addEventListener("keplr_keystorechange", onKeyStoreChange);

      if (window.localStorage.getItem('keplr-connected')) {
        await this.connectKeplr();
        return;
      } 

      document.addEventListener("DOMContentLoaded", () => htmx.trigger("#presales", "reload-all"));
    },

    truncatedAddress() {
      return this.wallet.account.address.replace(/(\w+1).*?(\w{4})$/, "$1...$2");
    },

    async connectKeplr() {
      await window.keplr.enable('pion-1');

      const offlineSigner = await window.getOfflineSigner('pion-1');

      const accounts = await offlineSigner.getAccounts();

      const account = accounts[0];
      
      const client = await SigningCosmWasmClient.connectWithSigner(
        "https://rpc-falcron.pion-1.ntrn.tech",
        offlineSigner,
      );

      this.connected = true;
      this.wallet = { offlineSigner, account, client };

      this.presaleGetModifier = (ev) => {
        ev.detail.path = ev.detail.path + `/${this.wallet.account.address}`;
      }

      document.body.addEventListener('htmx:configRequest', this.presaleGetModifier);

      window.localStorage.setItem('keplr-connected', true);

      htmx.trigger(window, "address-change");
      htmx.trigger("#presales", "reload-all");
    },

    async disconnectKeplr() {
      await window.keplr.disable('pion-1');

      document.body.removeEventListener('htmx:configRequest', this.presaleGetModifier);

      this.connected = false;
      this.address = '';
      this.wallet = null;

      window.localStorage.clear();

      htmx.trigger("#presales", "reload-all");
    }
  });

  Alpine.store('ops', {
    working: false,
    creatingShitcoin: false,
    extendingPresale: {},
    launchingShitcoin: {},
    claimingShitcoins: {},
    enteringPresale: {},

    toast(msg, type) {
      htmx.trigger('body', 'toast', { msg, type })
    },

    suppressToast(type) {
      window.localStorage.setItem(`suppress${type}`, true);
    },

    isToastSuppressed(type) {
      return window.localStorage.getItem(`suppress${type}`) != null
    },
  
    createShitcoin(wallet, params) {
      const createShitcoinMsg = {
        create_shitcoin: {
          ticker: params.ticker.trim(),
          name: params.name.trim(),
          supply: params.supply.replace(/,/g, ''),
        }
      };

      console.log("creating shitcoin:", createShitcoinMsg);

      const executeFee = calculateFee(750_000, gasPrice);

      wallet.client.execute(wallet.account.address, SHITCOIN_GARDEN, createShitcoinMsg, executeFee, "", [coin(10_000, "untrn")])
        .then(_ => {
          if (!this.isToastSuppressed('CreateShitcoin')) {
            this.toast(`The shitcoin $${params.ticker} has been created, I hope you're proud of yourself.`, 'CreateShitcoin');
          }
        })
        .catch(error => {
          this.toast(`Something went wrong: ${error}`);
        })
        .finally(_ => {
          this.working = false;
          this.creatingShitcoin = false;
        });

      this.creatingShitcoin = true;
      this.working = true;
    },

    isExtendingPresale(denom) {
      return this.extendingPresale[denom];
    },
    
    extendPresale(wallet, denom) {
      const extendPresaleMsg = {
        extend_presale: {
          denom
        }
      };

      console.log("extending shitcoin presale:", extendPresaleMsg);

      const executeFee = calculateFee(200_000, gasPrice);


      wallet.client.execute(wallet.account.address, SHITCOIN_GARDEN, extendPresaleMsg, executeFee)
        .then(_ => {
          if (!this.isToastSuppressed('ExtendPresale')) {
            this.toast("Presale extended another 24 hours, just let it die next time.", 'ExtendPresale');
          }
        })
        .catch(error => {
          this.toast(`Something went wrong: ${error}`);
        })
        .finally(_ => {
          this.extendingPresale[denom] = false;
          this.working = false;
        });

      this.extendingPresale[denom] = true;
      this.working = true;
    },

    isLaunchingShitcoin(denom) {
      return this.launchingShitcoin[denom];
    },
    
    launchShitcoin(wallet, denom) {
      const launchShitcoinMsg = {
        launch_shitcoin: {
          denom
        }
      };

      console.log("launch shitcoin:", launchShitcoinMsg);

      const executeFee = calculateFee(750_000, gasPrice);

      wallet.client.execute(wallet.account.address, SHITCOIN_GARDEN, launchShitcoinMsg, executeFee)
        .then(_ => {
          if (!this.isToastSuppressed('ShitcoinLaunched')) {
            this.toast("Shitcoin Launched!", 'ShitcoinLaunched');
          }
        })
        .catch(error => {
          this.toast(`Something went wrong: ${error}`);
        })
        .finally(_ => {
          this.launchingShitcoin[denom] = false;
          this.working = false;
        });

      this.launchingShitcoin[denom] = true;
      this.working = true;
    },

    isClaimingShitcoins(denom) {
      return this.claimingShitcoins[denom];
    },
    
    claimShitcoins(wallet, denom) {
      const claimShitcoinsMsg = {
        claim_shitcoin: {
          denom
        }
      };

      console.log("claiming shitcoins:", claimShitcoinsMsg);

      const executeFee = calculateFee(500_000, gasPrice);

      wallet.client.execute(wallet.account.address, SHITCOIN_GARDEN, claimShitcoinsMsg, executeFee)
        .then(_ => {
          if (!this.isToastSuppressed('ShitcoinsClaimed')) {
            this.toast("Shitcoins Claimed! Good luck, you'll be needing it.", 'ShitcoinsClaimed');
          }
        })
        .catch(error => {
          this.toast(`Something went wrong: ${error}`);
        })
        .finally(_ => {
          this.claimingShitcoins[denom] = false;
          this.working = false;
        });

      this.claimingShitcoins[denom] = true;
      this.working = true;
    },
    
    isEnteringPresale(denom) {
      return this.enteringPresale[denom];
    },

    enterPresale(wallet, denom, amount) {
      const enterPresaleMsg = {
        enter_presale: {
          denom
        }
      };


      const executeFee = calculateFee(500_000, gasPrice);

      const enterAmount = +amount.replace(/,/g, '') * 10**6;

      console.log(`entering presale with ${enterAmount}:`, enterPresaleMsg);

      wallet.client.execute(wallet.account.address, SHITCOIN_GARDEN, enterPresaleMsg, executeFee, "", [coin(enterAmount, PRESALE_DENOM)])
        .then(_ => {
          if (!this.isToastSuppressed('PresaleEntered')) {
            this.toast("Presale Entered! You brave (degenerate) soul", 'PresaleEntered');
          }
        })
        .catch(error => {
          this.toast(`Something went wrong: ${error}`);
        })
        .finally(_ => {
          this.enteringPresale[denom] = false;
          this.working = false;
        });

      this.enteringPresale[denom] = true;
      this.working = true;
    },

    setUrl(wallet, denom, url) {
      const setUrlMsgMsg = {
        set_url: {
          denom, 
          url
        }
      };


      const executeFee = calculateFee(500_000, gasPrice);

      wallet.client.execute(wallet.account.address, SHITCOIN_GARDEN, setUrlMsgMsg, executeFee)
        .then(_ => {
          if (!this.isToastSuppressed('UrlSet')) {
            this.toast("Url Set! Your coin is still shit.", 'UrlSet');
          }
        })
        .catch(error => {
          this.toast(`Something went wrong: ${error}`);
        })
        .finally(_ => {
          this.working = false;
        });

      this.working = true;
    },
  });

  Alpine.data('secondTick', () => ({
    init() {
      setInterval(() => {
        htmx.trigger(window, 'second-tick');
      }, 1000);
    },
  }));

  const HOURS_SECS = 60 * 60;
  const MINUTES_SECS = 60;
  
  Alpine.data('saleTimer', (expiry, ticker) => ({
    expiry: expiry,
    remaining: null,
    interval: null,
    ticker: ticker,
    init() {
      this.remaining = this.remainingSeconds();

      if (this.remaining === 0) {
        return;        
      }
      
      this.interval = setInterval(() => {
        this.tick();
      }, 1000);
    },
    remainingSeconds() {
      let remainingMillis = Math.max(this.expiry - Date.now(), 0);
      return Math.floor(remainingMillis / 1000);
    },
    tick() {
      this.remaining = this.remainingSeconds();

      if (this.remaining > 0) return;

      let id = `#presale-${this.ticker}`;

      setTimeout(() => {
        htmx.trigger(id, 'reload');
      }, 2000);

      window.clearInterval(this.interval);
    },
    hours() {
      return Math.floor(this.remaining / HOURS_SECS);
    },
    minutes() {
    	return Math.floor((this.remaining % HOURS_SECS) / MINUTES_SECS)
    },
    seconds() {
    	return Math.floor(((this.remaining % HOURS_SECS) % MINUTES_SECS))
    },
    format(value) {
      return String(value).padStart(2, '0')
    },
    time(){
    	return {
        hours: this.format(this.hours()),
        minutes:this.format(this.minutes()),
        seconds:this.format(this.seconds()),
      }
    },
  }));
})

