// Copyright (c) 2018 The Dash Core developers

#include "spork.h"
#include "string.h"
#include "chainparams.h"

#include "test/test_dash.h"
#include <boost/test/unit_test.hpp>

BOOST_FIXTURE_TEST_SUITE(spork_tests, BasicTestingSetup)

// private key for this WIF / address:
// 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
std::string sporkKey = "cTJTN1hGHqucsgqmYVbhU3g4eU9g5HzE1sxuSY32M1xap1K4sYHF";
std::string sporkAddr = "yMtMWAjPhUquwKtdG4wzj9Cpn4asQkLV8F";

// unable to construct a SporkManager 'til we fix a few things... :/
//CSporkManager HelperTestSporkManager(void) {
//    CSporkManager sporkMgr = CSporkManager();
//
//    if (!sporkMgr.SetPrivKey(sporkKey)) {
//        // TODO: return error("could not set spork privkey")
//        return sporkMgr;
//    }
//    if (!sporkMgr.SetSporkAddress(sporkAddr)) {
//        // TODO: return error("could not set spork address")
//        return sporkMgr;
//    }
//
//    return sporkMgr;
//}

BOOST_AUTO_TEST_CASE(spork_manager__GetSporkIDByName)
{
    SelectParams(CBaseChainParams::TESTNET);

    CSporkManager sporkMgr;
    sporkMgr.SetSporkAddress(sporkAddr);
    sporkMgr.SetPrivKey(sporkKey);

    std::map<std::string, int> tests = {
        {"SPORK_2_INSTANTSEND_ENABLED", 10001},
        {"SPORK_3_INSTANTSEND_BLOCK_FILTERING", 10002},
        {"SPORK_5_INSTANTSEND_MAX_VALUE", 10004},
        {"SPORK_6_NEW_SIGS", 10005},
        {"SPORK_8_MASTERNODE_PAYMENT_ENFORCEMENT", 10007},
        {"SPORK_9_SUPERBLOCKS_ENABLED", 10008},
        {"SPORK_10_MASTERNODE_PAY_UPDATED_NODES", 10009},
        {"SPORK_12_RECONSIDER_BLOCKS", 10011},
        {"SPORK_14_REQUIRE_SENTINEL_FLAG", 10013},
        {"SPORK_15_DETERMINISTIC_MNS_ENABLED", 10014},
        // {"SPORK_16_INSTANTSEND_AUTOLOCKS", 10015},
        {"ANYTHINGELSE", -1},
    };

    for (const auto& testPair : tests) {
        auto sporkID = sporkMgr.GetSporkIDByName(testPair.first);
        BOOST_CHECK_EQUAL(sporkID, testPair.second);
    }
}

BOOST_AUTO_TEST_CASE(spork_manager__SetPrivKey)
{
    SelectParams(CBaseChainParams::TESTNET);

    CSporkManager sporkMgr;
    assert(sporkMgr.SetSporkAddress(sporkAddr));
    BOOST_CHECK_EQUAL(sporkMgr.SetPrivKey(sporkKey), true);

    std::string wrongSporkKey = "cTsdXxTC346tsb7HaddDzC5dTqAT8XCsdJsacS4N3ak2mCGGZcN5";
    BOOST_CHECK_EQUAL(sporkMgr.SetPrivKey(wrongSporkKey), false);
}

//BOOST_AUTO_TEST_CASE(spork_message_test)
//{
//    // Example assertion
//    BOOST_CHECK_MESSAGE(tests.size(), "Empty `tests`");
//}


BOOST_AUTO_TEST_SUITE_END()
