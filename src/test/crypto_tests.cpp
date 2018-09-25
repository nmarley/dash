// Copyright (c) 2014-2015 The Bitcoin Core developers
// Distributed under the MIT software license, see the accompanying
// file COPYING or http://www.opensource.org/licenses/mit-license.php.

#include "crypto/aes.h"
#include "utilstrencodings.h"
#include "test/test_dash.h"
#include "test/test_random.h"

#include <vector>

#include <boost/assign/list_of.hpp>
#include <boost/test/unit_test.hpp>
#include <openssl/aes.h>
#include <openssl/evp.h>

BOOST_FIXTURE_TEST_SUITE(crypto_tests, BasicTestingSetup)

void TestAES256CBC(const std::string &hexkey, const std::string &hexiv, bool pad, const std::string &hexin, const std::string &hexout)
{
    std::vector<unsigned char> key = ParseHex(hexkey);
    std::vector<unsigned char> iv = ParseHex(hexiv);
    std::vector<unsigned char> in = ParseHex(hexin);
    std::vector<unsigned char> correctout = ParseHex(hexout);
    std::vector<unsigned char> realout(in.size() + AES_BLOCKSIZE);

    // Encrypt the plaintext and verify that it equals the cipher
    AES256CBCEncrypt enc(&key[0], &iv[0], pad);
    int size = enc.Encrypt(&in[0], in.size(), &realout[0]);
    realout.resize(size);
    BOOST_CHECK(realout.size() == correctout.size());
    BOOST_CHECK_MESSAGE(realout == correctout, HexStr(realout) + std::string(" != ") + hexout);

    // Decrypt the cipher and verify that it equals the plaintext
    std::vector<unsigned char> decrypted(correctout.size());
    AES256CBCDecrypt dec(&key[0], &iv[0], pad);
    size = dec.Decrypt(&correctout[0], correctout.size(), &decrypted[0]);
    decrypted.resize(size);
    BOOST_CHECK(decrypted.size() == in.size());
    BOOST_CHECK_MESSAGE(decrypted == in, HexStr(decrypted) + std::string(" != ") + hexin);

    // Encrypt and re-decrypt substrings of the plaintext and verify that they equal each-other
    for(std::vector<unsigned char>::iterator i(in.begin()); i != in.end(); ++i) {
        std::vector<unsigned char> sub(i, in.end());
        std::vector<unsigned char> subout(sub.size() + AES_BLOCKSIZE);
        int _size = enc.Encrypt(&sub[0], sub.size(), &subout[0]);
        if (_size != 0)
        {
            subout.resize(_size);
            std::vector<unsigned char> subdecrypted(subout.size());
            _size = dec.Decrypt(&subout[0], subout.size(), &subdecrypted[0]);
            subdecrypted.resize(_size);
            BOOST_CHECK(decrypted.size() == in.size());
            BOOST_CHECK_MESSAGE(subdecrypted == sub, HexStr(subdecrypted) + std::string(" != ") + HexStr(sub));
        }
    }
}

BOOST_AUTO_TEST_SUITE_END()
