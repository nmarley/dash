// Copyright (c) 2018 The Dash Core developers

#include "spork.h"
#include "string.h"
#include "chainparams.h"
#include "masternode.h"

#include "test/test_dash.h"
#include <boost/test/unit_test.hpp>

BOOST_FIXTURE_TEST_SUITE(mnp_tests, BasicTestingSetup)

// private key for this WIF / address:
// 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
std::string sporkKey = "cTJTN1hGHqucsgqmYVbhU3g4eU9g5HzE1sxuSY32M1xap1K4sYHF";
std::string sporkAddr = "yMtMWAjPhUquwKtdG4wzj9Cpn4asQkLV8F";

BOOST_AUTO_TEST_CASE(MNP_GetHash)
{
    // CMasternodePing
    mnp = CMasternodePing();
}

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
