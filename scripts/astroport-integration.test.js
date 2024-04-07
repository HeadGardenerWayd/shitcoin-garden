import { SigningCosmWasmClient }   from "@cosmjs/cosmwasm-stargate";
import { DirectSecp256k1HdWallet, coin } from "@cosmjs/proto-signing";
import { calculateFee, GasPrice }  from "@cosmjs/stargate";
import * as fs from "fs";

const WASM_FILE_PATH = `${__dirname}/../artifacts/shitcoin-garden.wasm`;

const walletMnemonic = process.env.TESTNET_WALLET_MNEMONIC;
const walletAddress  = process.env.TESTNET_WALLET_ADDRESS;
const chainPrefix    = process.env.TESTNET_CHAIN_PREFIX;
const chainDenom     = process.env.TESTNET_CHAIN_DENOM;
const chainRpc       = process.env.TESTNET_CHAIN_RPC;
const chainGasPrice  = process.env.TESTNET_CHAIN_GAS_PRICE;
const poolFactory    = process.env.TESTNET_POOL_FACTORY;

if (!walletMnemonic) throw new Error("TESTNET_WALLET_MNEMONIC env var not set");
if (!walletAddress)  throw new Error("TESTNET_WALLET_ADDRESS env var not set");
if (!chainPrefix)    throw new Error("TESTNET_CHAIN_PREFIX env var not set");
if (!chainDenom)     throw new Error("TESTNET_CHAIN_DENOM env var not set");
if (!chainRpc)       throw new Error("TESTNET_CHAIN_RPC env var not set");
if (!chainGasPrice)  throw new Error("TESTNET_CHAIN_GAS_PRICE env var not set");
if (!poolFactory)    throw new Error("TESTNET_POOL_FACTORY env var not set");

const gasPrice = GasPrice.fromString(chainGasPrice);
const wallet = await DirectSecp256k1HdWallet.fromMnemonic(walletMnemonic, { prefix: chainPrefix });
const client = await SigningCosmWasmClient.connectWithSigner(chainRpc, wallet);

const wasm = fs.readFileSync(WASM_FILE_PATH);
const uploadFee = calculateFee(2_500_000, gasPrice);
const uploadReceipt = await client.upload(walletAddress, wasm, uploadFee, "SG-IYKYK");

console.log("uploaded contract")

const initMsg = {
    pool_factory_address: poolFactory,
    fee_recipient: walletAddress,
    create_fee_denom: chainDenom,
    create_fee: "10000",
    presale_denom: chainDenom,
    presale_length: 20, // seconds
    presale_fee_rate: 50,
};
const instantiateFee = calculateFee(500_000, gasPrice);
const { contractAddress } = await client.instantiate(
  walletAddress,
  uploadReceipt.codeId,
  initMsg,
  "SG-IYKYK",
  instantiateFee,
);

console.log(`instantiated contract: ${contractAddress}`);

const executeFee = calculateFee(1_000_000, gasPrice);

const createShitcoinMsg = {
  create_shitcoin: {
    ticker: "TNT",
    name: "testnet terror",
    supply: "69420",
  }
};

await client.execute(walletAddress, contractAddress, createShitcoinMsg, executeFee, "", [coin(10000, chainDenom)]);

let denom = "";

{
  const shitcoins = await client.queryContractSmart(contractAddress, { shitcoins: {} });

  const shitcoin = shitcoins.shitcoins[0];

  console.log(`created shitcoin: ${shitcoin.denom}`);

  denom = shitcoin.denom;
}

const enterPresaleMsg = {
  enter_presale: { denom }
};

await client.execute(walletAddress, contractAddress, enterPresaleMsg, executeFee, "", [coin(100_000, chainDenom)]);

{
  const shitcoin = await client.queryContractSmart(contractAddress, { shitcoin_metadata: { denom } });

  console.log(`shitcoin presale entered: ${shitcoin.presale_raise}`);
}

console.log("waiting 20 seconds for presale to have ended");

await Bun.sleep(20_000);

const launchShitcoinMsg = {
  launch_shitcoin: { denom }
};

await client.execute(walletAddress, contractAddress, launchShitcoinMsg, executeFee);

{
  const shitcoin = await client.queryContractSmart(contractAddress, { shitcoin_metadata: { denom } });

  console.log(`shitcoin launched: ${shitcoin.launched}`);
}

const claimShitcoinMsg = {
  claim_shitcoin: { denom }
};

await client.execute(walletAddress, contractAddress, claimShitcoinMsg, executeFee);

const shitcoinBalance = await client.getBalance(walletAddress, denom);

console.log(`claimed shitcoin: ${shitcoinBalance.amount} ${shitcoinBalance.denom}`);

