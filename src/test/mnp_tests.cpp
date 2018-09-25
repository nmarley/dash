// Copyright (c) 2018 The Dash Core developers

#include "spork.h"
#include "string.h"
#include "chainparams.h"
#include "masternode.h"

#include "test/test_dash.h"
#include <boost/test/unit_test.hpp>

BOOST_FIXTURE_TEST_SUITE(mnp_tests, TestChain100Setup)

// private key for this WIF / address:
// 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
std::string sporkKey = "cTJTN1hGHqucsgqmYVbhU3g4eU9g5HzE1sxuSY32M1xap1K4sYHF";
std::string sporkAddr = "yMtMWAjPhUquwKtdG4wzj9Cpn4asQkLV8F";

BOOST_AUTO_TEST_CASE(MNP_GetHash)
{
    // COutPoint
    COutPoint x = COutPoint(uint256S("0x147caa76786596590baa4e98f5d9f48b86c7765e489f7a6ff3360fe5c674360b"), 0);
    const COutPoint& ref = x;
    CMasternodePing mnp = CMasternodePing(ref);

    //     CHashWriter ss(SER_GETHASH, PROTOCOL_VERSION);
    //     ss << masternodeOutpoint << uint8_t{} << 0xffffffff; // adding dummy values here to match old hashing format
    //     ss << sigTime;
    //     return ss.GetHash();

    // debugging
    //     ss << masternodeOutpoint << uint8_t{} << 0xffffffff; // adding dummy values here to match old hashing format
    // std::cout << "NGM DEBUGGING: mnp.masternodeOutpoint: " << mnp.masternodeOutpoint << std::endl;

    // class COutPoint {
    // public:
    //     uint256 hash;
    //     uint32_t n;

    std::cout << "NGM DEBUGGING: mnp.masternodeOutpoint.hash: " << mnp.masternodeOutpoint.hash.ToString() << std::endl;
    std::cout << "NGM DEBUGGING: mnp.masternodeOutpoint.n: " << mnp.masternodeOutpoint.n << std::endl;

    std::cout << "NGM DEBUGGING: mnp.sigTime: " << mnp.sigTime << std::endl;
    std::cout << "NGM DEBUGGING: mnp.GetHash(): " << mnp.GetHash().ToString() << std::endl;
}

// CMasternodePing::CMasternodePing(const COutPoint& outpoint) {
//    LOCK(cs_main);
//    if (!chainActive.Tip() || chainActive.Height() < 12) return;
//    masternodeOutpoint = outpoint;
//    blockHash = chainActive[chainActive.Height() - 12]->GetBlockHash();
//    sigTime = GetAdjustedTime();
//    nDaemonVersion = CLIENT_VERSION;
// }


//BOOST_AUTO_TEST_CASE(SporkManager__SetPrivKey)
//{
//     BOOST_CHECK_EQUAL(sporkMgr.SetPrivKey(sporkKey), true);
//}

//BOOST_AUTO_TEST_CASE(SporkMessage_test)
//{
//    // Example assertion
//    BOOST_CHECK_MESSAGE(tests.size(), "Empty `tests`");
//}


BOOST_AUTO_TEST_SUITE_END()
